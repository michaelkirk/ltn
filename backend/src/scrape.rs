use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::Result;
use geo::Coord;
use osm_reader::NodeID;
use utils::Tags;

use crate::{Direction, FilterKind, Intersection, IntersectionID, MapModel, Road, RoadID, Router};

struct ReadBarriers {
    all_barriers: BTreeMap<NodeID, Coord>,
    used_road_nodes: BTreeSet<NodeID>,
}

impl utils::osm2graph::OsmReader for ReadBarriers {
    fn node(&mut self, id: NodeID, pt: Coord, tags: Tags) {
        // Tuning these by hand for a few known areas.
        // https://wiki.openstreetmap.org/wiki/Key:barrier is proper reference.
        if let Some(kind) = tags.get("barrier") {
            // Bristol has many gates that don't seem as relevant
            if kind != "gate" {
                self.all_barriers.insert(id, pt);
            }
        }
    }

    fn way(
        &mut self,
        _: osm_reader::WayID,
        nodes: &Vec<NodeID>,
        _: &HashMap<NodeID, Coord>,
        tags: &Tags,
    ) {
        // Bit repetitive, but need to remember this to figure out which barriers are valid
        if is_road(tags) {
            self.used_road_nodes.extend(nodes.clone());
        }
    }
}

pub fn scrape_osm(input_bytes: &[u8], study_area_name: Option<String>) -> Result<MapModel> {
    let mut barriers = ReadBarriers {
        all_barriers: BTreeMap::new(),
        used_road_nodes: BTreeSet::new(),
    };
    let graph = utils::osm2graph::Graph::new(input_bytes, is_road, &mut barriers)?;

    // There'll be many barrier nodes on non-driveable paths we don't consider roads. Filter for
    // just those on things we consider roads.
    let mut barrier_pts = Vec::new();
    for (node, pt) in barriers.all_barriers {
        if barriers.used_road_nodes.contains(&node) {
            barrier_pts.push(graph.mercator.pt_to_mercator(pt));
        }
    }

    // Copy all the fields
    let intersections: Vec<Intersection> = graph
        .intersections
        .into_iter()
        .map(|i| Intersection {
            id: IntersectionID(i.id.0),
            point: i.point,
            node: i.osm_node,
            roads: i.edges.into_iter().map(|e| RoadID(e.0)).collect(),
        })
        .collect();

    // Add in a bit
    let roads: Vec<Road> = graph
        .edges
        .into_iter()
        .map(|e| Road {
            id: RoadID(e.id.0),
            src_i: IntersectionID(e.src.0),
            dst_i: IntersectionID(e.dst.0),
            way: e.osm_way,
            node1: e.osm_node1,
            node2: e.osm_node2,
            linestring: e.linestring,
            tags: e.osm_tags,
        })
        .collect();
    info!("Finalizing the map model");

    let mut directions = BTreeMap::new();
    for r in &roads {
        directions.insert(r.id, Direction::from_osm(&r.tags));
    }

    let mut map = MapModel {
        roads,
        intersections,
        mercator: graph.mercator,
        boundary_polygon: graph.boundary_polygon,
        study_area_name,

        router_original: None,
        router_current: None,
        router_original_with_penalty: None,

        original_modal_filters: BTreeMap::new(),
        modal_filters: BTreeMap::new(),

        directions,

        undo_stack: Vec::new(),
        redo_queue: Vec::new(),
        boundaries: BTreeMap::new(),
    };

    // Apply barriers (only those that're exactly on one of the roads)
    let all_roads: BTreeSet<RoadID> = map.roads.iter().map(|r| r.id).collect();
    for pt in barrier_pts {
        // TODO What kind?
        map.add_modal_filter(pt, &all_roads, FilterKind::NoEntry);
    }
    // The commands above populate the existing modal filters and edit history. Undo that.
    map.original_modal_filters = map.modal_filters.clone();
    map.undo_stack.clear();
    map.redo_queue.clear();

    let main_road_penalty = 1.0;
    map.router_original = Some(Router::new(
        &map.roads,
        &map.intersections,
        &map.modal_filters,
        &map.directions,
        main_road_penalty,
    ));

    Ok(map)
}

fn is_road(tags: &Tags) -> bool {
    if !tags.has("highway") || tags.is("area", "yes") {
        return false;
    }
    if tags.is_any(
        "highway",
        vec![
            "cycleway", "footway", "steps", "path", "track", "corridor", "proposed",
        ],
    ) {
        return false;
    }
    true
}
