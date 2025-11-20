window.addEventListener("pageshow", () => {
  let search = document.getElementsByClassName("search")[0];

  if (search) {
    search.value = "";
  }
});
