/** All UI strings live here (PLAN.md §6). */
export const ru = {
  appName: "Escape",
  tabs: {
    home: "Главная",
    clipboard: "Буфер",
    launcher: "Команды",
    settings: "Настройки",
  },
  tabsLabel: "Разделы острова",
  comingSoon: "Скоро",
  battery: {
    label: (percent: number) => `Батарея ${String(percent)}%`,
    charging: "заряжается",
    pluggedIn: "подключена к сети",
  },
} as const;
