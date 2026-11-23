/*
 * MapLibre GL JS <-> Dioxus bridge.
 *
 * The Dioxus app calls these methods through document::eval. This script emits
 * CustomEvents on `window` that Rust listens for:
 *   map-ready        - the map finished loading and layers are installed
 *   event-selected   - detail: event id (string) - a dot was clicked
 *   map-moved        - detail: {west,south,east,north} - viewport changed
 *   map-clicked      - detail: {lat,lng} - user tapped the map (for submission)
 */
window.EventMap = {
  map: null,

  // A minimal raster style built on OpenStreetMap tiles. Attribution is shown.
  // For production replace with a Protomaps .pmtiles file or MapTiler style URL.
  osmStyle() {
    return {
      version: 8,
      // Glyphs are required so the cluster-count symbol layer can render numbers.
      glyphs: "https://demotiles.maplibre.org/font/{fontstack}/{range}.pbf",
      sources: {
        osm: {
          type: "raster",
          tiles: ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"],
          tileSize: 256,
          attribution:
            '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
        },
      },
      layers: [{ id: "osm", type: "raster", source: "osm" }],
    };
  },

  init(containerId, center, zoom) {
    if (this.map) return;
    this.map = new maplibregl.Map({
      container: containerId,
      style: this.osmStyle(),
      center: center,
      zoom: zoom,
      attributionControl: true,
    });
    this.map.addControl(new maplibregl.NavigationControl(), "top-right");
    this.map.addControl(new maplibregl.GeolocateControl({
      positionOptions: { enableHighAccuracy: true },
      trackUserLocation: true,
    }), "top-right");

    this.map.on("load", () => {
      this.map.addSource("events", {
        type: "geojson",
        data: { type: "FeatureCollection", features: [] },
        cluster: true,
        clusterRadius: 50,
        clusterMaxZoom: 14,
      });

      // Cluster bubbles
      this.map.addLayer({
        id: "clusters",
        type: "circle",
        source: "events",
        filter: ["has", "point_count"],
        paint: {
          "circle-color": "#1B2A2F",
          "circle-radius": 18,
          "circle-stroke-width": 3,
          "circle-stroke-color": "#fff",
        },
      });
      this.map.addLayer({
        id: "cluster-count",
        type: "symbol",
        source: "events",
        filter: ["has", "point_count"],
        layout: { "text-field": ["get", "point_count_abbreviated"], "text-size": 13 },
        paint: { "text-color": "#fff" },
      });

      // Single event dots, colored by category.
      this.map.addLayer({
        id: "dots",
        type: "circle",
        source: "events",
        filter: ["!", ["has", "point_count"]],
        paint: {
          "circle-radius": 11,
          "circle-stroke-width": 3,
          "circle-stroke-color": "#fff",
          "circle-color": [
            "match", ["get", "category"],
            "music", "#C2410C",
            "food", "#1D4ED8",
            "arts", "#7C3AED",
            "community", "#0F766E",
            "#444444",
          ],
        },
      });

      this.map.on("click", "dots", (e) => {
        const id = e.features[0].properties.id;
        window.dispatchEvent(new CustomEvent("event-selected", { detail: id }));
      });
      this.map.on("click", "clusters", (e) => {
        const f = e.features[0];
        this.map.getSource("events")
          .getClusterExpansionZoom(f.properties.cluster_id)
          .then((z) => this.map.easeTo({ center: f.geometry.coordinates, zoom: z }));
      });
      this.map.on("moveend", () => {
        const b = this.map.getBounds();
        window.dispatchEvent(new CustomEvent("map-moved", {
          detail: {
            west: b.getWest(), south: b.getSouth(),
            east: b.getEast(), north: b.getNorth(),
          },
        }));
      });
      // Tap-to-place for the submission form.
      this.map.on("click", (e) => {
        window.dispatchEvent(new CustomEvent("map-clicked", {
          detail: { lat: e.lngLat.lat, lng: e.lngLat.lng },
        }));
      });

      window.dispatchEvent(new CustomEvent("map-ready"));
    });
  },

  setEvents(events) {
    if (!this.map || !this.map.getSource("events")) return;
    const fc = {
      type: "FeatureCollection",
      features: events.map((e) => ({
        type: "Feature",
        geometry: { type: "Point", coordinates: [e.lng, e.lat] },
        properties: { id: e.id, category: e.category, title: e.title },
      })),
    };
    this.map.getSource("events").setData(fc);
  },

  flyTo(lat, lng, zoom) {
    if (!this.map) return;
    this.map.flyTo({ center: [lng, lat], zoom: zoom || 14, essential: true });
  },

  // Enable/disable the tap-to-place mode used by the submit form.
  setPickMode(enabled) {
    if (!this.map) return;
    if (enabled) this.map.getCanvas().style.cursor = "crosshair";
    else this.map.getCanvas().style.cursor = "";
  },
};
