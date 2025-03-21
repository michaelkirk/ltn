<script lang="ts">
  import type { Feature, Polygon } from "geojson";
  import type { AreaProps } from "route-snapper-ts";
  import { notNull } from "svelte-utils";
  import { gjPosition, Link } from "./common";
  import AreaControls from "./common/draw_area/AreaControls.svelte";
  import { calculateArea, waypoints } from "./common/draw_area/stores";
  import { autosave, backend, editPerimeterRoads, map, mode } from "./stores";

  export let name: string;
  export let existing: Feature<Polygon, AreaProps> | null;

  if (existing) {
    if (existing.properties.waypoints) {
      // Transform into the correct format
      $waypoints = existing.properties.waypoints.map((waypt) => {
        return {
          point: [waypt.lon, waypt.lat],
          snapped: waypt.snapped,
        };
      });
    } else {
      // No stored waypoints -- this is either a boundary drawn with a very old
      // version of this tool, or an auto-generated boundary. Just
      // "backfill" by using the full geometry as freehand points.
      // Editing will be very painful in practice, but it won't break.
      $waypoints = existing.geometry.coordinates[0].slice(1).map((point) => {
        return { point: gjPosition(point), snapped: false };
      });
    }
  }

  function finish() {
    if ($waypoints.length >= 3) {
      try {
        let feature = calculateArea($waypoints);
        $backend!.setNeighbourhoodBoundary(name, feature);
        autosave();
        $backend!.setCurrentNeighbourhood(name, $editPerimeterRoads);
        $mode = {
          mode: "neighbourhood",
        };
      } catch (err) {
        window.alert(
          "Known georust bug hit, sorry. You may need to just refresh the page now.",
        );
        cancel();
      }
    }
  }

  function cancel() {
    if (existing) {
      $mode = {
        mode: "neighbourhood",
      };
    } else {
      $mode = {
        mode: "pick-neighbourhood",
      };
    }
  }
</script>

<AreaControls map={notNull($map)} {finish} {cancel}>
  <div slot="extra-top">
    <nav aria-label="breadcrumb">
      <ul>
        <li>
          <Link on:click={() => ($mode = { mode: "title", firstLoad: false })}>
            Choose project
          </Link>
        </li>
        <li>
          <Link on:click={() => ($mode = { mode: "pick-neighbourhood" })}>
            Pick neighbourhood
          </Link>
        </li>
        {#if existing}
          <li>
            <Link on:click={() => ($mode = { mode: "neighbourhood" })}>
              Editing
            </Link>
          </li>
          <li>Changing neighbourhood boundary</li>
        {:else}
          <li>Creating new neighbourhood boundary</li>
        {/if}
      </ul>
    </nav>
  </div>

  <div slot="extra-sidebar">
    <h1>Draw your neighbourhood boundary for {name}</h1>
  </div>
</AreaControls>
