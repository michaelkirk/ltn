import type { FeatureCollection, MultiPolygon, Polygon } from "geojson";
import boundariesUrl from "../../assets/cnt_boundaries.geojson?url";

export type CNTStudyAreaKind = "LAD" | "REGION";
export class CNTStudyAreas {
  regionNames: string[] = [];
  ladNames: string[] = [];
  gj: FeatureCollection<
    Polygon | MultiPolygon,
    { kind: CNTStudyAreaKind; name: string }
  > = {
    type: "FeatureCollection",
    features: [],
  };

  async load() {
    let resp = await fetch(boundariesUrl);
    console.assert(this.regionNames.length == 0);
    console.assert(this.ladNames.length == 0);
    console.assert(this.gj.features.length == 0);

    this.gj = await resp.json();
    for (let f of this.gj.features) {
      if (f.properties.kind == "LAD") {
        this.ladNames.push(f.properties.name);
      } else {
        this.regionNames.push(f.properties.name);
      }
    }
    this.ladNames.sort();
    this.regionNames.sort();
  }
}
