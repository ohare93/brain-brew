const config = {
  title: 'Brain Brew',
  tagline: 'Local-first deck federation for Anki-compatible shared decks',
  url: 'https://jeprecated.github.io',
  baseUrl: '/brain-brew/',
  organizationName: 'jeprecated',
  projectName: 'brain-brew',

  onBrokenLinks: 'throw',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          routeBasePath: '/',
          sidebarPath: './sidebars.js',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      },
    ],
  ],

  themeConfig: {
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Brain Brew',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docs',
          position: 'left',
          label: 'Docs',
        },
        {
          href: 'https://github.com/jeprecated/brain-brew',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Learn',
          items: [
            { label: 'Quick start', to: '/getting-started/quickstart' },
            { label: 'Overlay kinds', to: '/concepts/overlays' },
            { label: 'Field fills', to: '/authoring/field-fills' },
          ],
        },
        {
          title: 'Reference',
          items: [
            { label: 'CLI', to: '/reference/cli' },
            { label: 'YAML formats', to: '/reference/yaml' },
            { label: 'ADRs', to: '/reference/decisions/' },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Brain Brew contributors.`,
    },
    prism: {
      additionalLanguages: ['bash', 'yaml', 'rust'],
    },
  },
};

module.exports = config;
