import { defineConfig } from "vitepress";

const repo = "https://github.com/InDIOS/gpui-bridge";

export default defineConfig({
  title: "gpui-bridge",
  description: "Native GPUI desktop UIs driven from Python, Node.js, React, Go or any language that can start a process.",
  // GitHub Pages serves the project at https://indios.github.io/gpui-bridge/.
  base: "/gpui-bridge/",
  cleanUrls: true,
  head: [["link", { rel: "icon", type: "image/svg+xml", href: "/gpui-bridge/favicon.svg" }]],
  markdown: { theme: { light: "github-light", dark: "github-dark" } },
  themeConfig: {
    logo: "/logo.svg",
    nav: [
      { text: "Guide", link: "/guide/how-it-works" },
      { text: "Clients", link: "/clients/python" },
      { text: "Protocol", link: "/protocol" },
      { text: "New client", link: "/guide/new-client" },
      { text: "Changelog", link: "/changelog" },
    ],
    sidebar: [
      {
        text: "Guide",
        items: [
          { text: "How it works", link: "/guide/how-it-works" },
          { text: "Writing a new client", link: "/guide/new-client" },
        ],
      },
      {
        text: "Clients",
        items: [
          { text: "Python", link: "/clients/python" },
          { text: "Node.js and txiki.js", link: "/clients/node" },
          { text: "React", link: "/clients/react" },
          { text: "Go", link: "/clients/go" },
        ],
      },
      {
        text: "Protocol",
        items: [
          { text: "Specification (v1)", link: "/protocol" },
        ],
      },
      {
        text: "Project",
        items: [
          { text: "Changelog", link: "/changelog" },
          { text: "Contributing", link: "/project/contributing" },
          { text: "Releasing", link: "/project/releasing" },
        ],
      },
    ],
    outline: { level: [2, 3] },
    search: { provider: "local" },
    socialLinks: [{ icon: "github", link: repo }],
    footer: {
      message: "Released under the MIT License.",
      copyright: "Built on GPUI and gpui-kit.",
    },
  },
});
