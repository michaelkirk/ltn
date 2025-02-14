<script lang="ts">
  import {
    FillLayer,
    GeoJSON,
    hoverStateFilter,
    LineLayer,
    type LayerClickInfo,
  } from "svelte-maplibre";
  import { CNTStudyAreas, type CNTStudyAreaKind } from "./CNTStudyAreas";

  export var studyAreaKind: CNTStudyAreaKind;

  // Not sure why this isn't working
  export var gj: CNTStudyAreas["gj"];
  // export var gj: any;
  function onClick(e: CustomEvent<LayerClickInfo>) {
    console.assert(false, "TODO: implement onClick");
    // let props = e.detail.features[0].properties!;
    // newFile(`${props.kind}_${props.name}`);
  }
</script>

<GeoJSON data={gj} generateId>
  <FillLayer
    filter={["==", ["get", "kind"], studyAreaKind]}
    paint={{
      "fill-color": "rgb(200, 100, 240)",
      "fill-outline-color": "rgb(200, 100, 240)",
      "fill-opacity": hoverStateFilter(0.0, 0.5),
    }}
    beforeId="Road labels"
    manageHoverState
    hoverCursor="pointer"
    on:click={onClick}
  >
    <!-- FIXME: <Popup openOn="hover" let:props>
      <p>{props.name}</p>
    </Popup> -->
  </FillLayer>

  <LineLayer
    filter={["==", ["get", "kind"], studyAreaKind]}
    paint={{
      "line-color": "rgb(200, 100, 240)",
      "line-width": 2.5,
    }}
    beforeId="Road labels"
    manageHoverState
  />
</GeoJSON>
