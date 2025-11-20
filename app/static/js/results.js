// TODO: When inputting a query, and then going back, the query is still in the input box
window.addEventListener("pageshow", () => {
  const params = new URLSearchParams(window.location.search);

  const query = params.get("q");

  let search = document.getElementsByClassName("search")[0];

  // If the parameter exists, set it as the value of the input
  if (query && search) {
    search.value = query;
  }
});
