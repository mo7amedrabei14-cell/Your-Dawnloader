import { defineConfig } from "wxt";

export default defineConfig({
  manifest: {
    name: "Your Dawnloader — Media Capture",
    description: "Find directly accessible media candidates on the active page.",
    permissions: ["activeTab", "scripting", "downloads", "storage"],
    action: {
      default_title: "Scan for media",
      default_popup: "popup.html",
    },
  },
});
