<script lang="ts">
  import { type DataDrivenPropertyValueSpecification } from "maplibre-gl";
  import { FillLayer, LineLayer, SymbolLayer } from "svelte-maplibre";
  import { layerId } from "../common";
  import { roadStyle } from "../stores";

  function colorIconByCell(): DataDrivenPropertyValueSpecification<string> {
    return [
      "match",
      ["get", "cell_color"],
      "disconnected",
      "red",
      // For numeric values, need to use a step function with % operation
      [
        "let",
        "index",
        ["%", ["to-number", ["get", "cell_color"]], 10],
        [
          "match",
          ["var", "index"],
          0,
          "#8dd3c7",
          1,
          "#ffffb3",
          2,
          "#bebada",
          3,
          "#80b1d3",
          4,
          "#fdb462",
          5,
          "#b3de69",
          6,
          "#fccde5",
          7,
          "#bc80bd",
          8,
          "#ccebc5",
          9,
          "#ffed6f",
          "blue", // default color if something goes wrong
        ],
      ],
    ];
  }

  function borderEntryIconSize(
    extraWidth: number,
  ): DataDrivenPropertyValueSpecification<number> {
    return [
      "interpolate",
      ["linear"],
      ["zoom"],
      14,
      0.15 + extraWidth,
      18,
      0.15 + extraWidth,
    ];
  }
</script>

<FillLayer
  {...layerId("cells")}
  filter={["==", ["get", "kind"], "cell"]}
  layout={{
    visibility: $roadStyle == "cells" ? "none" : "visible",
  }}
  paint={{
    "fill-color": ["get", "color"],
    "fill-opacity": 0.5,
    "fill-outline-color": "hsla(0, 0%, 0%, 0.3)",
  }}
/>

<FillLayer
  {...layerId("border-arrows")}
  filter={["==", ["get", "kind"], "border_arrow"]}
  paint={{
    "fill-color": ["get", "color"],
    "fill-opacity": 0.7,
  }}
  minzoom={13}
/>
<LineLayer
  {...layerId("border-arrow-outlines")}
  filter={["==", ["get", "kind"], "border_arrow"]}
  paint={{
    "line-color": "black",
    "line-width": 2,
  }}
  minzoom={13}
/>

<SymbolLayer
  {...layerId("border-entries")}
  filter={["==", ["get", "kind"], "border_entry"]}
  layout={{
    "icon-image": "border_entry_arrow",
    "icon-rotate": ["get", "bearing_upon_entry"],
    "icon-allow-overlap": true,
    "icon-size": borderEntryIconSize(0),
  }}
  paint={{
    "icon-color": colorIconByCell(),
  }}
  minzoom={13}
  interactive={false}
/>

<SymbolLayer
  {...layerId("border-entries-outline")}
  filter={["==", ["get", "kind"], "border_entry"]}
  layout={{
    "icon-image": "border_entry_arrow",
    "icon-rotate": ["get", "bearing_upon_entry"],
    "icon-allow-overlap": true,
    // This looks bad... maybe I need to do some custom scaling in an image editor and save an external icon.
    "icon-size": borderEntryIconSize(0.1),
  }}
  paint={{
    "icon-color": "black",
  }}
  minzoom={13}
  interactive={false}
/>
