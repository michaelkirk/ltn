<script lang="ts">
  import { GeoJSON, SymbolLayer } from "svelte-maplibre";
  import { emptyGeojson } from "svelte-utils/map";
  import { layerId } from "../common";
  import { backend, mutationCounter } from "../stores";

  // TODO Runes would make this so nicer. The > 0 part is a hack...
  $: gj =
    $mutationCounter > 0 ? $backend!.renderModalFilters() : emptyGeojson();
</script>

<GeoJSON data={gj} generateId>
  <SymbolLayer
    {...layerId("modal-filters")}
    filter={["!=", ["get", "filter_kind"], "diagonal_filter"]}
    layout={{
      "icon-image": ["get", "filter_kind"],
      "icon-rotate": ["get", "angle"],
      "icon-allow-overlap": true,
      "icon-size": 0.1,
    }}
    paint={{
      "icon-opacity": ["case", ["get", "edited"], 1.0, 0.5],
    }}
    on:click
  >
    <slot />
  </SymbolLayer>
  <SymbolLayer
    {...layerId("intersection-filters")}
    filter={["==", ["get", "filter_kind"], "diagonal_filter"]}
    layout={{
      "icon-image": "diagonal_filter",
      "icon-rotate": ["get", "angle", ["get", "filter"]],
      "icon-allow-overlap": true,
      "icon-size": 0.07,
    }}
    interactive={false}
  />
</GeoJSON>
