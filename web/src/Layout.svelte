<div class="container">
  <div class="top">
    <slot name="top" />
  </div>
  <div class="content">
    <div class="left">
      <slot name="left" />
    </div>
    <div class="main">
      <slot name="main" />
    </div>
  </div>
</div>

<style>
  .container {
    display: flex;
    flex-direction: column;
    height: 100vh; /* Full viewport height */
  }

  .top {
    width: 100%;
    background: lightgray; /* Optional styling for visualization */
  }

  .content {
    display: flex;
    flex: 1; /* Takes up remaining space */
    overflow: hidden; /* Prevents horizontal overflow */
  }

  .left {
    width: 25%;
    background: #f0f0f0; /* Optional styling for visualization */
    overflow-y: auto; /* Allows vertical scrolling */
    padding: 8px;
  }

  .main {
    width: 75%;
    background: #fff; /* Optional styling for visualization */
    padding: 8px;
    overflow-y: auto; /* Allows vertical scrolling */
  }
</style>
