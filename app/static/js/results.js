window.addEventListener("DOMContentLoaded", () => {
  const params = new URLSearchParams(window.location.search);

  const query = params.get("q");

  let search = document.getElementsByClassName("search")[0];

  // If the parameter exists, set it as the value of the input
  if (query && search) {
    search.value = query;
  }
});
