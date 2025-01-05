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
    margin: 0;
  }

  .top {
    width: 100%;
  }

  .content {
    display: flex;
    flex: 1; /* Takes up remaining space */
    overflow: hidden; /* Prevents horizontal overflow */
  }

  .left {
    width: 25%;
    overflow-y: auto; /* Allows vertical scrolling */
    padding: 8px;
  }

  .main {
    width: 75%;
  }
</style>
