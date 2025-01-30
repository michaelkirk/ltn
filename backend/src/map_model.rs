use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use crate::geo_helpers::{
    angle_of_pt_on_line, bearing_from_endpoint, buffer_aabb, diagonal_bearing, invert_polygon,
    limit_angle, linestring_intersection,
};
use crate::impact::Impact;
use crate::Router;
use anyhow::Result;
use geo::{
    Closest, ClosestPoint, Coord, Euclidean, Length, Line, LineInterpolatePoint, LineLocatePoint,
    LineString, Point, Polygon,
};
use geojson::{Feature, FeatureCollection, GeoJson, Geometry, JsonValue};
use rstar::{primitives::GeomWithData, RTree, AABB};
use serde::Serialize;
use utils::{osm2graph, Mercator, Tags};

pub struct MapModel {
    pub roads: Vec<Road>,
    pub intersections: Vec<Intersection>,
    pub bus_routes_on_roads: HashMap<osm_reader::WayID, Vec<String>>,
    // All geometry stored in worldspace, including rtrees
    pub mercator: Mercator,
    pub study_area_name: Option<String>,
    pub boundary_wgs84: Polygon,
    pub closest_road: RTree<GeomWithData<LineString, RoadID>>,
    pub closest_intersection: RTree<GeomWithData<Point, IntersectionID>>,

    // Only those acting as severances; above or belowground don't count
    pub railways: Vec<LineString>,
    pub waterways: Vec<LineString>,

    // TODO Wasteful, can share some
    // This is guaranteed to exist, only Option during MapModel::new internals
    pub router_before: Option<Router>,
    // Calculated lazily. Changes with edits and main_road_penalty.
    pub router_after: Option<Router>,
    // Calculated lazily. No edits, just main_road_penalty.
    pub router_before_with_penalty: Option<Router>,

    // Just from the basemap, existing filters
    pub original_modal_filters: BTreeMap<RoadID, ModalFilter>,
    pub modal_filters: BTreeMap<RoadID, ModalFilter>,
    pub diagonal_filters: BTreeMap<IntersectionID, DiagonalFilter>,

    // Every road is filled out
    pub directions: BTreeMap<RoadID, Direction>,

    pub impact: Option<Impact>,

    // TODO Keep edits / state here or not?
    pub undo_stack: Vec<Command>,
    pub redo_queue: Vec<Command>,
    // Stores boundary polygons in WGS84, with ALL of their GeoJSON props.
    // TODO Reconsider
    pub boundaries: BTreeMap<String, Feature>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize)]
pub struct RoadID(pub usize);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize)]
pub struct IntersectionID(pub usize);

impl fmt::Display for RoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Road #{}", self.0)
    }
}

impl fmt::Display for IntersectionID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Intersection #{}", self.0)
    }
}

/// A segment of a road network - no intersections happen *within* a `Road`.
/// An osm Way is divided into potentially multiple `Road`s
#[derive(Debug, Clone)]
pub struct Road {
    pub id: RoadID,
    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,
    pub way: osm_reader::WayID,
    pub linestring: LineString,
    pub tags: Tags,
    pub speed_mph: usize,
}

/// Connection between `Road` (segments).
#[derive(Debug, Clone)]
pub struct Intersection {
    pub id: IntersectionID,
    pub node: osm_reader::NodeID,
    pub point: Point,
    // Ordered clockwise from North
    pub roads: Vec<RoadID>,
    /// (from, to) is not allowed. May be redundant with the road directions.
    pub turn_restrictions: Vec<(RoadID, RoadID)>,
}

impl Intersection {
    pub(crate) fn from_graph(mut value: osm2graph::Intersection, roads: &[Road]) -> Self {
        // Sort intersection roads clockwise, starting from North
        value.edges.sort_by_cached_key(|road_id| {
            let road = &roads[road_id.0];
            let bearing = bearing_from_endpoint(value.point, &road.linestring);
            // work around that f64 is not Ord
            debug_assert!(
                bearing.is_finite(),
                "Assuming bearing output is always 0...360, this shouldn't happen"
            );
            (bearing * 1e6) as i64
        });

        Intersection {
            id: IntersectionID(value.id.0),
            point: value.point,
            node: value.osm_node,
            roads: value.edges.into_iter().map(|e| RoadID(e.0)).collect(),
            turn_restrictions: Vec::new(),
        }
    }

    pub fn roads_iter<'a>(&'a self, map: &'a MapModel) -> impl Iterator<Item = &'a Road> {
        self.roads.iter().map(move |road_id| map.get_r(*road_id))
    }
}

impl MapModel {
    /// Call with bytes of an osm.pbf or osm.xml string
    pub fn new(
        input_bytes: &[u8],
        boundary_wgs84: Polygon,
        study_area_name: Option<String>,
    ) -> Result<MapModel> {
        crate::create::create_from_osm(input_bytes, boundary_wgs84, study_area_name)
    }

    pub fn get_r(&self, r: RoadID) -> &Road {
        &self.roads[r.0]
    }

    pub fn get_i(&self, i: IntersectionID) -> &Intersection {
        &self.intersections[i.0]
    }

    pub fn find_edge(&self, i1: IntersectionID, i2: IntersectionID) -> &Road {
        // TODO Store lookup table
        for r in &self.get_i(i1).roads {
            let road = self.get_r(*r);
            if road.src_i == i2 || road.dst_i == i2 {
                return road;
            }
        }
        panic!("no road from {i1} to {i2} or vice versa");
    }

    pub fn add_modal_filter(
        &mut self,
        pt: Coord,
        candidate_roads: Option<Vec<RoadID>>,
        kind: FilterKind,
    ) {
        let cmd = self.do_edit(self.add_modal_filter_cmd(pt, candidate_roads, kind));
        self.undo_stack.push(cmd);
        self.redo_queue.clear();
        self.after_edited();
    }

    fn add_modal_filter_cmd(
        &self,
        pt: Coord,
        candidate_roads: Option<Vec<RoadID>>,
        mut kind: FilterKind,
    ) -> Command {
        let (r, percent_along) = self.closest_point_on_road(pt, candidate_roads).unwrap();
        if self.get_bus_routes_on_road(r).is_some() && kind != FilterKind::BusGate {
            info!("Using a BusGate instead of {kind:?} for a road");
            kind = FilterKind::BusGate;
        }
        Command::SetModalFilter(
            r,
            Some(ModalFilter {
                percent_along,
                kind,
            }),
        )
    }

    fn closest_point_on_road(
        &self,
        click_pt: Coord,
        candidate_roads: Option<Vec<RoadID>>,
    ) -> Option<(RoadID, f64)> {
        // If candidate_roads is not specified, search around the point with a generous buffer
        let roads = candidate_roads.unwrap_or_else(|| {
            let bbox = buffer_aabb(AABB::from_point(click_pt.into()), 50.0);
            self.closest_road
                .locate_in_envelope_intersecting(&bbox)
                .map(|r| r.data)
                .collect()
        });

        roads
            .into_iter()
            .filter_map(|r| {
                let road = self.get_r(r);
                if let Some(hit_pt) = match road.linestring.closest_point(&click_pt.into()) {
                    Closest::Intersection(pt) => Some(pt),
                    Closest::SinglePoint(pt) => Some(pt),
                    Closest::Indeterminate => None,
                } {
                    let score = Line::new(click_pt, hit_pt.into()).length::<Euclidean>();
                    let percent_along = road.linestring.line_locate_point(&hit_pt).unwrap();
                    Some(((score * 100.0) as usize, road.id, percent_along))
                } else {
                    None
                }
            })
            .min_by_key(|pair| pair.0)
            .map(|pair| (pair.1, pair.2))
    }

    fn most_similar_linestring(&self, linestring: &LineString) -> RoadID {
        // TODO Detect many possible cases of OSM data changing. Could at least compare the length
        // of the candidate. Decide how to handle possible splits/merges.
        self.roads
            .iter()
            .min_by_key(|r| {
                let diff1 = Line::new(
                    r.linestring.points().next().unwrap(),
                    linestring.points().next().unwrap(),
                )
                .length::<Euclidean>();
                let diff2 = Line::new(
                    r.linestring.points().last().unwrap(),
                    linestring.points().last().unwrap(),
                )
                .length::<Euclidean>();
                ((diff1 + diff2) * 100.0) as usize
            })
            .unwrap()
            .id
    }

    fn after_edited(&mut self) {
        self.router_after = None;
        self.impact.as_mut().unwrap().invalidate_after_edits();
    }

    pub fn add_many_modal_filters(
        &mut self,
        along_line: LineString,
        candidate_roads: &BTreeSet<RoadID>,
        kind: FilterKind,
    ) {
        let mut edits = Vec::new();
        for r in candidate_roads {
            let road = self.get_r(*r);
            if let Some(percent_along) = linestring_intersection(&road.linestring, &along_line) {
                let mut use_kind = kind;
                if self.get_bus_routes_on_road(*r).is_some() && kind != FilterKind::BusGate {
                    info!("Using a BusGate instead of {kind:?} for a road");
                    use_kind = FilterKind::BusGate;
                }

                edits.push(Command::SetModalFilter(
                    *r,
                    Some(ModalFilter {
                        percent_along,
                        kind: use_kind,
                    }),
                ));
            }
        }
        let cmd = self.do_edit(Command::Multiple(edits));
        self.undo_stack.push(cmd);
        self.redo_queue.clear();
        self.after_edited();
    }

    pub fn delete_modal_filter(&mut self, r: RoadID) {
        let cmd = self.do_edit(Command::SetModalFilter(r, None));
        self.undo_stack.push(cmd);
        self.redo_queue.clear();
        self.after_edited();
    }

    pub fn add_diagonal_filter(&mut self, i: IntersectionID) {
        let intersection = self.get_i(i);
        let diagonal_filter = DiagonalFilter::new(intersection, 0, self);
        let cmd = Command::SetDiagonalFilter(i, Some(diagonal_filter));
        let undo_cmd = self.do_edit(cmd);
        self.undo_stack.push(undo_cmd);
        self.redo_queue.clear();
        self.after_edited();
    }

    pub fn rotate_diagonal_filter(&mut self, i: IntersectionID) {
        let intersection = self.get_i(i);
        let diagonal_filter = DiagonalFilter::new(intersection, 1, self);
        let cmd = Command::SetDiagonalFilter(i, Some(diagonal_filter));
        let undo_cmd = self.do_edit(cmd);
        self.undo_stack.push(undo_cmd);
        self.redo_queue.clear();
        self.after_edited();
    }

    pub fn delete_diagonal_filter(&mut self, i: IntersectionID) {
        let cmd = Command::SetDiagonalFilter(i, None);
        let undo_cmd = self.do_edit(cmd);
        self.undo_stack.push(undo_cmd);
        self.redo_queue.clear();
        self.after_edited();
    }

    pub fn toggle_direction(&mut self, r: RoadID) {
        let dir = match self.directions[&r] {
            Direction::Forwards => Direction::Backwards,
            Direction::Backwards => Direction::BothWays,
            Direction::BothWays => Direction::Forwards,
        };
        let cmd = self.do_edit(Command::SetDirection(r, dir));
        self.undo_stack.push(cmd);
        self.redo_queue.clear();
        self.after_edited();
    }

    // Returns the command to undo this one
    fn do_edit(&mut self, cmd: Command) -> Command {
        match cmd {
            Command::SetModalFilter(r, filter) => {
                let prev = self.modal_filters.get(&r).cloned();
                if let Some(filter) = filter {
                    info!("added a filter to {r} at {}%", filter.percent_along);
                    self.modal_filters.insert(r, filter);
                } else {
                    info!("deleted a filter from {r}");
                    self.modal_filters.remove(&r);
                }
                Command::SetModalFilter(r, prev)
            }
            Command::SetDiagonalFilter(i, filter) => {
                let prev = self.diagonal_filters.get(&i).cloned();
                if let Some(filter) = filter {
                    info!("added filter to {i:?}: {filter:?}");
                    self.diagonal_filters.insert(i, filter);
                } else {
                    let filter = self.diagonal_filters.remove(&i);
                    info!("removed filter from {i:?}: {filter:?}");
                }
                Command::SetDiagonalFilter(i, prev)
            }
            Command::SetDirection(r, dir) => {
                info!("changed direction of {r} to {}", dir.to_string());
                let prev = self.directions[&r];
                self.directions.insert(r, dir);
                Command::SetDirection(r, prev)
            }
            Command::Multiple(list) => {
                let undo_list = list.into_iter().map(|cmd| self.do_edit(cmd)).collect();
                Command::Multiple(undo_list)
            }
        }
    }

    pub fn undo(&mut self) {
        // The UI shouldn't call this when the stack is empty, but when holding down the redo key,
        // it doesn't update fast enough
        if let Some(cmd) = self.undo_stack.pop() {
            let cmd = self.do_edit(cmd);
            self.redo_queue.push(cmd);
            self.after_edited();
        }
    }

    pub fn redo(&mut self) {
        if self.redo_queue.is_empty() {
            return;
        }
        let cmd = self.redo_queue.remove(0);
        let cmd = self.do_edit(cmd);
        self.undo_stack.push(cmd);
        self.after_edited();
    }

    pub fn filters_to_gj(&self) -> FeatureCollection {
        let mut features = Vec::new();
        for (r, filter) in &self.modal_filters {
            let road = self.get_r(*r);
            let pt = road
                .linestring
                .line_interpolate_point(filter.percent_along)
                .unwrap();
            let angle = limit_angle(angle_of_pt_on_line(&road.linestring, pt.into()) + 90.0);
            let mut f = self.mercator.to_wgs84_gj(&pt);
            f.set_property("filter_kind", filter.kind.to_string());
            f.set_property("road", r.0);
            f.set_property("angle", angle);
            f.set_property("edited", Some(filter) != self.original_modal_filters.get(r));
            features.push(f);
        }
        FeatureCollection {
            features,
            bbox: None,
            foreign_members: None,
        }
    }

    /// Because ids like RoadID and IntersectionID aren't guaranteed to be stable across loads,
    /// we use more permanent markers like GPS points to map to features.
    pub fn to_savefile(&self) -> FeatureCollection {
        // Edited filters only
        let mut gj = self.filters_to_gj();
        gj.features
            .retain(|f| f.property("edited").unwrap().as_bool().unwrap());
        for f in &mut gj.features {
            f.set_property("kind", "modal_filter");
            f.remove_property("road");
        }

        // Look for any basemap filters that were deleted entirely
        for (r, filter) in &self.original_modal_filters {
            if self.modal_filters.contains_key(r) {
                continue;
            }
            let pt = self
                .get_r(*r)
                .linestring
                .line_interpolate_point(filter.percent_along)
                .unwrap();
            let mut f = self.mercator.to_wgs84_gj(&pt);
            f.set_property("kind", "deleted_existing_modal_filter");
            gj.features.push(f);
        }

        // Any direction edits
        for r in &self.roads {
            if self.directions[&r.id] != Direction::from_osm(&r.tags) {
                let mut f = self.mercator.to_wgs84_gj(&r.linestring);
                f.set_property("kind", "direction");
                f.set_property("direction", self.directions[&r.id].to_string());
                gj.features.push(f);
            }
        }

        gj.features.extend(self.boundaries.values().cloned());

        let mut f = Feature::from(Geometry::from(&self.boundary_wgs84));
        f.set_property("kind", "study_area_boundary");
        gj.features.push(f);

        for (i, filter) in &self.diagonal_filters {
            let intersection = self.get_i(*i);
            let mut f = self.mercator.to_wgs84_gj(&intersection.point);
            f.set_property("kind", "diagonal_filter");
            let split_offset = intersection
                .roads
                .iter()
                .position(|el| el == &filter.group_a[0])
                .expect("filter must contain roads that belong to intersection");
            f.set_property("split_offset", split_offset);
            gj.features.push(f);
        }

        gj.foreign_members = Some(
            serde_json::json!({
                "study_area_name": self.study_area_name,
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        gj
    }

    pub fn load_savefile(&mut self, gj: FeatureCollection) -> Result<()> {
        // Clear previous state
        self.boundaries.clear();
        self.modal_filters = self.original_modal_filters.clone();
        for (r, dir) in &mut self.directions {
            *dir = Direction::from_osm(&self.roads[r.0].tags);
        }
        self.undo_stack.clear();
        self.redo_queue.clear();

        // Filters could be defined for multiple neighbourhoods, not just the one
        // in the savefile
        let mut cmds = Vec::new();

        for f in gj.features {
            match f
                .property("kind")
                .expect("savefile feature missing `kind`")
                .as_str()
                .unwrap()
            {
                "modal_filter" => {
                    let kind = FilterKind::from_string(get_str_prop(&f, "filter_kind")?)?;
                    let gj_pt: Point = f.geometry.unwrap().try_into()?;
                    cmds.push(self.add_modal_filter_cmd(
                        self.mercator.pt_to_mercator(gj_pt.into()),
                        None,
                        kind,
                    ));
                }
                "deleted_existing_modal_filter" => {
                    let gj_pt: Point = f.geometry.unwrap().try_into()?;
                    let pt = self.mercator.pt_to_mercator(gj_pt.into());
                    // TODO Better error handling if we don't match
                    let (r, _) = self.closest_point_on_road(pt, None).unwrap();
                    cmds.push(Command::SetModalFilter(r, None));
                }
                "direction" => {
                    let dir = Direction::from_string(get_str_prop(&f, "direction")?)?;
                    let mut linestring: LineString = f.geometry.unwrap().try_into()?;
                    self.mercator.to_mercator_in_place(&mut linestring);
                    let r = self.most_similar_linestring(&linestring);
                    cmds.push(Command::SetDirection(r, dir));
                }
                "boundary" => {
                    let name = get_str_prop(&f, "name")?;
                    if self.boundaries.contains_key(name) {
                        bail!("Multiple boundaries named {name} in savefile");
                    }
                    self.boundaries.insert(name.to_string(), f);
                }
                "study_area_boundary" => {
                    // TODO Detect if it's close enough to boundary_polygon? Overwrite?
                }
                "diagonal_filter" => {
                    let i = {
                        let gj_pt: Point = f.geometry.as_ref().unwrap().try_into()?;
                        let pt = self.mercator.pt_to_mercator(gj_pt.into());
                        self.closest_intersection
                            .nearest_neighbor(&Point(pt))
                            .expect("intersection near saved editable intersection")
                            .data
                    };
                    let intersection = self.get_i(i);
                    let split_offset = f
                        .property("split_offset")
                        .expect("missing split_offset")
                        .as_u64()
                        .expect("unsigned integer");

                    let diagonal_filter =
                        DiagonalFilter::new(intersection, split_offset as usize, self);
                    self.diagonal_filters
                        .insert(intersection.id, diagonal_filter);
                }
                x => bail!("Unknown kind in savefile: {x}"),
            }
        }

        // Keep the undo stack empty. A user shouldn't be able to undo and clear the whole
        // savefile.
        self.do_edit(Command::Multiple(cmds));
        self.after_edited();

        Ok(())
    }

    // Lazily builds the router if needed.
    pub fn rebuild_router(&mut self, main_road_penalty: f64) {
        if self
            .router_after
            .as_ref()
            .map(|r| r.main_road_penalty != main_road_penalty)
            .unwrap_or(true)
        {
            self.router_after = Some(Router::new(
                &self.roads,
                &self.modal_filters,
                &self.directions,
                main_road_penalty,
            ));
        }

        if self
            .router_before_with_penalty
            .as_ref()
            .map(|r| r.main_road_penalty != main_road_penalty)
            .unwrap_or(true)
        {
            self.router_before_with_penalty = Some(Router::new(
                &self.roads,
                &self.original_modal_filters,
                &self.original_directions(),
                main_road_penalty,
            ));
        }
    }

    pub fn compare_route(&mut self, pt1: Coord, pt2: Coord, main_road_penalty: f64) -> GeoJson {
        self.rebuild_router(main_road_penalty);

        let mut features = Vec::new();
        if let Some(route) = self
            .router_before_with_penalty
            .as_ref()
            .unwrap()
            .route(self, pt1, pt2)
        {
            let (distance, time) = route.get_distance_and_time(self);
            let mut f = self.mercator.to_wgs84_gj(&route.to_linestring(self));
            f.set_property("kind", "before");
            f.set_property("distance", distance);
            f.set_property("time", time);
            features.push(f);
        }
        if let Some(route) = self.router_after.as_ref().unwrap().route(self, pt1, pt2) {
            let (distance, time) = route.get_distance_and_time(self);
            let mut f = self.mercator.to_wgs84_gj(&route.to_linestring(self));
            f.set_property("kind", "after");
            f.set_property("distance", distance);
            f.set_property("time", time);
            features.push(f);
        }
        GeoJson::from(features)
    }

    pub fn impact_to_one_destination(
        &mut self,
        pt2: Coord,
        from: Vec<RoadID>,
    ) -> FeatureCollection {
        // main_road_penalty doesn't seem relevant for this question
        self.rebuild_router(1.0);

        // From every road, calculate the route before and after to the one destination
        let mut features = Vec::new();
        let mut highest_time_ratio: f64 = 1.0;
        for r in from {
            let road = self.get_r(r);
            let pt1 = road.linestring.line_interpolate_point(0.5).unwrap().into();

            // TODO How to handle missing one or both routes missing?
            if let (Some(before), Some(after)) = (
                self.router_before_with_penalty
                    .as_ref()
                    .unwrap()
                    .route(self, pt1, pt2),
                self.router_after.as_ref().unwrap().route(self, pt1, pt2),
            ) {
                let from_pt = self.mercator.pt_to_wgs84(pt1);
                let (distance_before, time_before) = before.get_distance_and_time(self);
                let (distance_after, time_after) = after.get_distance_and_time(self);

                let mut f = self.mercator.to_wgs84_gj(&road.linestring);
                f.set_property("distance_before", distance_before);
                f.set_property("distance_after", distance_after);
                f.set_property("time_before", time_before);
                f.set_property("time_after", time_after);
                f.set_property("pt1_x", from_pt.x);
                f.set_property("pt1_y", from_pt.y);
                features.push(f);

                highest_time_ratio = highest_time_ratio.max(time_after / time_before);
            }
        }

        FeatureCollection {
            features,
            bbox: None,
            foreign_members: Some(
                serde_json::json!({
                    "highest_time_ratio": highest_time_ratio,
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        }
    }

    /// Return a polygon covering the world, minus a hole for the study area boundary, in WGS84
    pub fn invert_study_area_boundary(&self) -> Polygon {
        invert_polygon(self.boundary_wgs84.clone())
    }

    /// What're the names of bus routes along a road?
    pub fn get_bus_routes_on_road(&self, r: RoadID) -> Option<&Vec<String>> {
        let way = self.get_r(r).way;
        self.bus_routes_on_roads.get(&way)
    }

    fn original_directions(&self) -> BTreeMap<RoadID, Direction> {
        let mut directions = BTreeMap::new();
        for r in &self.roads {
            directions.insert(r.id, Direction::from_osm(&r.tags));
        }
        directions
    }
}

impl Road {
    // How long does it take for a car following the speed limit to cross this road?
    pub fn cost_seconds(&self) -> f64 {
        let meters = self.linestring.length::<Euclidean>();
        let meters_per_second = (self.speed_mph as f64) * 0.44704;
        meters / meters_per_second
    }

    pub fn to_gj(&self, mercator: &Mercator) -> Feature {
        let mut f = mercator.to_wgs84_gj(&self.linestring);
        f.set_property("id", self.id.0);
        f.set_property("speed_mph", self.speed_mph);
        // TODO Debug only, reconsider
        f.set_property("way", self.way.to_string());
        for (k, v) in &self.tags.0 {
            f.set_property(k, v.to_string());
        }
        f
    }
}

#[derive(Clone, PartialEq)]
pub struct ModalFilter {
    pub kind: FilterKind,
    pub percent_along: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DiagonalFilter {
    pub angle: f64,
    pub group_a: Vec<RoadID>,
    pub group_b: Vec<RoadID>,
}

/// A DiagonalFilter is placed at a 4-way intersection, and prevents traffic from going "straight"
/// through the intersection. Traffic must turn.
///
/// The DiagonalFilter can be placed in one of two rotations to determine which way traffic is forced
/// to turn.
///
/// Note: When all the roads at the intersection are 1-way roads, there is only one reasonable
/// orientation for the diagonal filter, the other orientation would effectively block the intersection.
/// We could choose to enforce "reasonable" filtering in the UI, or keep the interface consistent
/// and leave it up to the user to manually ensure the filter is orientated reasonably.
impl DiagonalFilter {
    /// Precondition: Intersection must be a 4-way intersection
    fn new(
        intersection: &Intersection,
        split_offset: usize,
        map_model: &MapModel,
    ) -> DiagonalFilter {
        debug_assert_eq!(
            intersection.roads.len(),
            4,
            "diagonal filters only support 4-way intersections"
        );

        let group_a: Vec<RoadID> = (0..2)
            .into_iter()
            .map(|offset| intersection.roads[(offset + split_offset) % intersection.roads.len()])
            .collect();

        let group_b: Vec<RoadID> = (2..4)
            .into_iter()
            .map(|offset| intersection.roads[(offset + split_offset) % intersection.roads.len()])
            .collect();

        let road_1 = map_model.get_r(group_a[0]);
        let road_2 = map_model.get_r(group_a[1]);

        let bearing_1 = bearing_from_endpoint(intersection.point, &road_1.linestring);
        let bearing_2 = bearing_from_endpoint(intersection.point, &road_2.linestring);
        let diagonal_angle = diagonal_bearing(bearing_1, bearing_2);
        DiagonalFilter {
            angle: diagonal_angle,
            group_a,
            group_b,
        }
    }

    // `movement`: (from, to)
    pub fn allows_movement(&self, movement: &(RoadID, RoadID)) -> bool {
        let (from, to) = movement;

        debug_assert!(self.group_a.contains(from) || self.group_b.contains(from));
        debug_assert!(self.group_a.contains(to) || self.group_b.contains(to));

        self.group_a.contains(from) && self.group_a.contains(to)
            || self.group_b.contains(from) && self.group_b.contains(to)
    }
}

impl From<&DiagonalFilter> for JsonValue {
    fn from(value: &DiagonalFilter) -> Self {
        serde_json::to_value(value).expect("valid JSON fields")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterKind {
    WalkCycleOnly,
    NoEntry,
    BusGate,
    SchoolStreet,
}

// TODO strum?
impl FilterKind {
    pub fn to_string(self) -> &'static str {
        match self {
            Self::WalkCycleOnly => "walk_cycle_only",
            Self::NoEntry => "no_entry",
            Self::BusGate => "bus_gate",
            Self::SchoolStreet => "school_street",
        }
    }

    pub fn from_string(x: &str) -> Result<Self> {
        match x {
            "walk_cycle_only" => Ok(Self::WalkCycleOnly),
            "no_entry" => Ok(Self::NoEntry),
            "bus_gate" => Ok(Self::BusGate),
            "school_street" => Ok(Self::SchoolStreet),
            _ => bail!("Invalid FilterKind: {x}"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Direction {
    Forwards,
    Backwards,
    BothWays,
}

impl Direction {
    pub fn from_osm(tags: &Tags) -> Self {
        // TODO Improve this
        if tags.is("oneway", "yes") {
            Self::Forwards
        } else if tags.is("oneway", "-1") {
            Self::Backwards
        } else {
            // https://wiki.openstreetmap.org/wiki/Key:oneway#Implied_oneway_restriction
            if tags.is("highway", "motorway") || tags.is("junction", "roundabout") {
                return Self::Forwards;
            }

            Self::BothWays
        }
    }

    // TODO strum?
    pub fn to_string(self) -> &'static str {
        match self {
            Self::Forwards => "forwards",
            Self::Backwards => "backwards",
            Self::BothWays => "both",
        }
    }

    pub fn from_string(x: &str) -> Result<Self> {
        match x {
            "forwards" => Ok(Self::Forwards),
            "backwards" => Ok(Self::Backwards),
            "both" => Ok(Self::BothWays),
            _ => bail!("Invalid Direction: {x}"),
        }
    }
}

pub enum Command {
    SetModalFilter(RoadID, Option<ModalFilter>),
    SetDiagonalFilter(IntersectionID, Option<DiagonalFilter>),
    SetDirection(RoadID, Direction),
    Multiple(Vec<Command>),
}

fn get_str_prop<'a>(f: &'a Feature, key: &str) -> Result<&'a str> {
    let Some(value) = f.property(key) else {
        bail!("Feature doesn't have a {key} property");
    };
    let Some(string) = value.as_str() else {
        bail!("Feature's {key} property isn't a string");
    };
    Ok(string)
}
