<script lang="ts">
  import { Pencil, Trash2 } from "lucide-svelte";
  import { Loading } from "svelte-utils";
  import { SplitComponent } from "svelte-utils/top_bar_layout";
  import CntChooseArea from "../CntChooseArea.svelte";
  import { Link, type ProjectID } from "../common";
  import { routeTool } from "../common/draw_area/stores";
  import {
    appFocus,
    backend,
    currentProjectID,
    map,
    mode,
    projectStorage,
  } from "../stores";
  import { loadProject } from "./loader";
  import LoadSavedProject from "./LoadSavedProject.svelte";

  export let wasmReady: boolean;
  export let firstLoad: boolean;

  let loading = "";

  // When other modes reset here, they can't clear state without a race condition
  {
    $backend = null;
    $routeTool = null;
    $currentProjectID = undefined;

    if (firstLoad) {
      let params = new URLSearchParams(window.location.search);
      let projectKey = params.get("project");
      if (projectKey) {
        loadProjectFromURLParam(projectKey);
      }
    } else {
      // Update the URL
      let url = new URL(window.location.href);
      url.searchParams.delete("project");
      window.history.replaceState(null, "", url.toString());
    }
  }

  let projectList = $projectStorage.listProjects();

  function deleteProjectPrompt(projectID: ProjectID, projectName: string) {
    if (
      window.confirm(
        `Really delete project ${projectName}? You can't undo this.`,
      )
    ) {
      $projectStorage.removeProject(projectID);
      projectList = $projectStorage.listProjects();
    }
  }

  function renameProjectPrompt(projectID: ProjectID, existingName: string) {
    let newName = window.prompt(
      `Rename project ${existingName} to what?`,
      existingName,
    );
    if (newName) {
      try {
        $projectStorage.renameProject(projectID, newName);
      } catch (e) {
        window.alert(`Couldn't rename project: ${e}`);
      }
      projectList = $projectStorage.listProjects();
    }
  }

  async function loadProjectFromURLParam(projectIDParam: string) {
    // TODO: verify it looks like a UUID
    let projectID = projectIDParam as ProjectID;
    let projectName = $projectStorage.projectName(projectID);
    if (!projectName) {
      console.error(`Project ${projectID} from URL not found`);
      return;
    }
    loadProjectPrompt(projectID, projectName);
  }

  async function loadProjectPrompt(projectID: ProjectID, projectName: string) {
    loading = `Loading project ${projectName}`;
    await loadProject(projectID);
    loading = "";
  }

  function prettyPrintStudyAreaName(studyAreaName: string | undefined): string {
    if (!studyAreaName) {
      return "custom area";
    }
    if ($appFocus == "cnt") {
      return studyAreaName.replace("LAD_", "");
    } else {
      return studyAreaName;
    }
  }
</script>

<Loading {loading} />

<SplitComponent>
  <div slot="top">
    <nav aria-label="breadcrumb">
      <ul>
        <li>Choose project</li>
      </ul>
    </nav>
  </div>
  <div slot="sidebar">
    {#if $map && wasmReady}
      {#if projectList.length > 0}
        <h2>Your projects</h2>
        <div class="project-list">
          {#each projectList as [studyAreaName, projects]}
            <h3 class="study-area-name">
              {prettyPrintStudyAreaName(studyAreaName)}
            </h3>
            <ul class="navigable-list">
              {#each projects as { projectID, projectName }}
                <li class="actionable-cell">
                  <h3>
                    <Link
                      on:click={() => loadProjectPrompt(projectID, projectName)}
                    >
                      {projectName}
                    </Link>
                  </h3>
                  <span class="actions">
                    <button
                      class="outline icon-btn"
                      aria-label="Rename project"
                      on:click={() =>
                        renameProjectPrompt(projectID, projectName)}
                    >
                      <Pencil color="black" />
                    </button>
                    <button
                      class="icon-btn destructive"
                      aria-label="Delete project"
                      on:click={() =>
                        deleteProjectPrompt(projectID, projectName)}
                    >
                      <Trash2 color="white" />
                    </button>
                  </span>
                </li>
              {/each}
            </ul>
          {/each}
        </div>
      {/if}

      <h2>Start a new project</h2>
      {#if $appFocus == "global"}
        <button on:click={() => ($mode = { mode: "new-project" })}>
          New project
        </button>
      {:else if $appFocus == "cnt"}
        <CntChooseArea bind:activityIndicatorText={loading} />
      {/if}
      <LoadSavedProject bind:loading />
    {:else}
      <p>Waiting for MapLibre and WASM to load...</p>
    {/if}
  </div>
</SplitComponent>

<style>
  .study-area-name {
    border-bottom: 1px solid #444;
  }
  .project-list {
    margin-top: 18px;
  }
  .project-list li {
    padding-left: 1em;
  }
</style>
