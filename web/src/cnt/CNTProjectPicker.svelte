<script lang="ts">
  import { Link } from "../common";

  export var studyAreaKind: string;
  export let regionNames: string[];
  export let ladNames: string[];

  function newFile(boundary: string) {
    let filename = "";
    while (true) {
      filename =
        window.prompt(
          `Please pick a project name to create in ${boundary}`,
          filename,
        ) || "";
      if (filename == "") {
        // If the user leaves this blank or presses cancel, stop prompting them.
        return;
      }
      let project = `ltn_cnt/${boundary}/${filename}`;
      if (window.localStorage.getItem(project) != null) {
        window.alert(
          `The project name ${filename} is already used; please pick another`,
        );
        continue;
      }

      // Create a blank project
      window.localStorage.setItem(
        project,
        JSON.stringify({
          type: "FeatureCollection",
          features: [],
          study_area_name: boundary,
        }),
      );

      window.location.href = `index.html?project=${encodeURIComponent(project)}`;
      return;
    }
  }

  // Returns boundary => list of filenames
  function listAllFiles(): Map<string, string[]> {
    let map = new Map();
    for (let i = 0; i < window.localStorage.length; i++) {
      let key = window.localStorage.key(i)!;
      if (key.startsWith("ltn_cnt/")) {
        try {
          let [_, boundary, filename] = key.split("/");
          if (!map.has(boundary)) {
            map.set(boundary, []);
          }
          map.get(boundary).push(filename);
        } catch (_) {}
      }
    }

    for (let list of map.values()) {
      list.sort();
    }
    return map;
  }
</script>

<fieldset>
  <label>
    <input type="radio" value="LAD" bind:group={studyAreaKind} />
    Local Authority Districts
  </label>
  <label>
    <input type="radio" value="REGION" bind:group={studyAreaKind} />
    Regions
  </label>
</fieldset>

<p>Choose a boundary below or on the map to begin sketching:</p>
<ul style="columns: 3">
  {#if studyAreaKind == "LAD"}
    {#each ladNames as name}
      <li><Link on:click={() => newFile(`LAD_${name}`)}>{name}</Link></li>
    {/each}
  {:else}
    {#each regionNames as name}
      <li>
        <Link on:click={() => newFile(`REGION_${name}`)}>{name}</Link>
      </li>
    {/each}
  {/if}
</ul>

<hr />

<p>Or continue with a previously opened file:</p>

<div style="columns: 2">
  {#each listAllFiles() as [boundary, list]}
    <div class="group">
      <h2>{boundary}</h2>
      {#each list as filename}
        <p>
          <a href={`index.html?project=ltn_cnt/${boundary}/${filename}`}>
            {filename}
          </a>
        </p>
      {/each}
    </div>
  {/each}
</div>

<style>
  .group {
    border: 1px solid black;
    padding: 4px;
    margin-bottom: 8px;
    break-inside: avoid-column;
  }
</style>
