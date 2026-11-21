use iced::widget::{
    button, column, container, row, rule, scrollable, text, Space,
};
use iced::{Alignment, Border, Color, Element, Length, Theme};

const BG: Color = Color::from_rgb8(12, 17, 25);
const SIDEBAR: Color = Color::from_rgb8(14, 20, 29);
const PANEL: Color = Color::from_rgb8(21, 29, 41);
const PANEL_HOVER: Color = Color::from_rgb8(27, 38, 52);
const HERO: Color = Color::from_rgb8(24, 39, 55);
const LINE: Color = Color::from_rgb8(39, 53, 69);
const TEXT: Color = Color::from_rgb8(226, 235, 245);
const MUTED: Color = Color::from_rgb8(139, 156, 176);
const ACCENT: Color = Color::from_rgb8(111, 205, 222);
const ACCENT_DARK: Color = Color::from_rgb8(18, 43, 54);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Instances,
    Catalog,
    Modpacks,
    Downloads,
    Accounts,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformChoice {
    Java,
    Horizon,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    SelectPlatform(PlatformChoice),
    Play,
    CreateInstance,
}

#[derive(Debug)]
pub struct App {
    page: Page,
    platform: PlatformChoice,
    notice: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            page: Page::Home,
            platform: PlatformChoice::Java,
            notice: None,
        }
    }
}

pub fn update(app: &mut App, message: Message) {
    match message {
        Message::Navigate(page) => {
            app.page = page;
            app.notice = None;
        }
        Message::SelectPlatform(platform) => {
            app.platform = platform;
            app.notice = Some(match platform {
                PlatformChoice::Java => "Выбрана платформа Minecraft Java".into(),
                PlatformChoice::Horizon => "Выбрана платформа Horizon".into(),
            });
        }
        Message::Play => {
            app.notice = Some("Игровой запуск появится после подключения менеджера экземпляров.".into());
        }
        Message::CreateInstance => {
            app.page = Page::Instances;
            app.notice = Some("Создание игровых экземпляров будет добавлено следующим этапом.".into());
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    row![
        container(sidebar(app))
            .width(Length::Fixed(236.0))
            .height(Length::Fill)
            .padding([24, 16])
            .style(|_| panel_style(SIDEBAR, Color::TRANSPARENT)),
        column![
            top_bar(app.page),
            scrollable(page_content(app))
                .height(Length::Fill)
                .width(Length::Fill),
        ]
        .height(Length::Fill)
        .width(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let brand = row![
        container(text("✦").size(23).color(ACCENT))
            .width(42)
            .height(42)
            .center(Length::Fill)
            .style(|_| panel_style(ACCENT_DARK, Color::TRANSPARENT)),
        column![
            text("GLOWSTONE").size(16).color(TEXT),
            text("CLIENT").size(10).color(ACCENT),
        ]
        .spacing(1),
    ]
    .spacing(11)
    .align_y(Alignment::Center);

    let library = column![
        section_label("БИБЛИОТЕКА"),
        nav_button("⌂", "Главная", Page::Home, app.page),
        nav_button("▦", "Мои сборки", Page::Instances, app.page),
    ]
    .spacing(5);

    let discover = column![
        section_label("ОТКРЫТЬ"),
        nav_button("◇", "Каталог модов", Page::Catalog, app.page),
        nav_button("▤", "Модпаки", Page::Modpacks, app.page),
    ]
    .spacing(5);

    let system = column![
        section_label("СИСТЕМА"),
        nav_button("⇩", "Загрузки", Page::Downloads, app.page),
        nav_button("♙", "Аккаунты", Page::Accounts, app.page),
        nav_button("⚙", "Настройки", Page::Settings, app.page),
    ]
    .spacing(5);

    column![
        brand,
        Space::new().height(34),
        library,
        Space::new().height(18),
        discover,
        Space::new().height(18),
        system,
        Space::new().height(Length::Fill),
        rule::horizontal(1).style(|_| rule::Style {
            color: LINE,
            ..Default::default()
        }),
        row![
            container(text("U").size(13).color(ACCENT))
                .width(34)
                .height(34)
                .center(Length::Fill)
                .style(|_| panel_style(ACCENT_DARK, Color::TRANSPARENT)),
            column![
                text("Локальный профиль").size(11).color(TEXT),
                text("Не авторизован").size(10).color(MUTED),
            ]
            .spacing(3),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    ]
    .spacing(0)
    .height(Length::Fill)
    .into()
}

fn top_bar(page: Page) -> Element<'static, Message> {
    let (title, subtitle) = page_title(page);

    row![
        column![
            text(title).size(19).color(TEXT),
            text(subtitle).size(11).color(MUTED),
        ]
        .spacing(3),
        Space::new().width(Length::Fill),
        container(
            row![
                text("●").size(9).color(ACCENT),
                text("Система готова").size(11).color(MUTED),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([9, 13])
        .style(|_| panel_style(PANEL, LINE)),
    ]
    .padding([22, 30])
    .align_y(Alignment::Center)
    .into()
}

fn page_title(page: Page) -> (&'static str, &'static str) {
    match page {
        Page::Home => ("Главная", "Ваша игровая библиотека"),
        Page::Instances => ("Мои сборки", "Управление игровыми экземплярами"),
        Page::Catalog => ("Каталог модов", "Находите моды и дополнения"),
        Page::Modpacks => ("Модпаки", "Готовые сборки и импорт"),
        Page::Downloads => ("Загрузки", "Очередь и история загрузок"),
        Page::Accounts => ("Аккаунты", "Управление игровыми профилями"),
        Page::Settings => ("Настройки", "Параметры GlowstoneCL"),
    }
}

fn page_content(app: &App) -> Element<'_, Message> {
    let content = match app.page {
        Page::Home => home_page(app),
        Page::Instances => empty_page(
            "Твои игровые сборки",
            "Создавай независимые экземпляры с разными версиями Minecraft, загрузчиками и наборами модов.",
            "Пока здесь пусто",
            "＋  Создать первую сборку",
            Message::CreateInstance,
        ),
        Page::Catalog => empty_page(
            "Открой для себя новые моды",
            "Modrinth, CurseForge и каталог нативных модов Horizon будут собраны в одном месте.",
            "Каталоги ещё не подключены",
            "На главную",
            Message::Navigate(Page::Home),
        ),
        Page::Modpacks => empty_page(
            "Модпаки",
            "Импортируй готовые сборки или собирай собственные коллекции модов.",
            "Модпаков пока нет",
            "Перейти к сборкам",
            Message::Navigate(Page::Instances),
        ),
        Page::Downloads => empty_page(
            "Центр загрузок",
            "Здесь будут отображаться скорость, прогресс, проверка файлов и история операций.",
            "Нет активных загрузок",
            "Открыть каталог",
            Message::Navigate(Page::Catalog),
        ),
        Page::Accounts => empty_page(
            "Твои аккаунты",
            "Подключай Microsoft или создавай локальные профили для offline-режима Java.",
            "Аккаунты ещё не добавлены",
            "Настроить позже",
            Message::Navigate(Page::Settings),
        ),
        Page::Settings => settings_page(),
    };

    container(content)
        .width(Length::Fill)
        .padding([12, 30, 30, 30])
        .into()
}

fn home_page(app: &App) -> Element<'_, Message> {
    let selected = match app.platform {
        PlatformChoice::Java => ("MINECRAFT JAVA", "Java Edition", "Vanilla и модифицированные версии"),
        PlatformChoice::Horizon => ("HORIZON · RUST", "Horizon", "Нативный Minecraft runtime на Rust"),
    };

    let hero = container(
        row![
            column![
                row![
                    container(text("●").size(8).color(ACCENT))
                        .padding([5, 8])
                        .style(|_| panel_style(ACCENT_DARK, Color::TRANSPARENT)),
                    text("ЕДИНЫЙ ЦЕНТР УПРАВЛЕНИЯ").size(10).color(ACCENT),
                ]
                .align_y(Alignment::Center)
                .spacing(8),
                Space::new().height(12),
                text("Твой Minecraft.").size(32).color(TEXT),
                text("Твои правила.").size(32).color(TEXT),
                text("Запускай любимые версии, собирай модпаки и управляй модами из одного места.")
                    .size(13)
                    .color(MUTED),
                Space::new().height(10),
                row![
                    accent_button("＋  Создать сборку", Message::CreateInstance),
                    quiet_button("Открыть каталог", Message::Navigate(Page::Catalog)),
                ]
                .spacing(10),
            ]
            .spacing(4)
            .width(Length::FillPortion(7)),
            container(
                column![
                    text("✦").size(72).color(ACCENT),
                    text("GLOWSTONE").size(12).color(TEXT),
                    text("UNIVERSAL LAUNCHER").size(9).color(MUTED),
                ]
                .spacing(5)
                .align_x(Alignment::Center),
            )
            .width(Length::FillPortion(3))
            .height(190)
            .center(Length::Fill)
            .style(|_| panel_style(Color::from_rgb8(19, 51, 67), Color::TRANSPARENT)),
        ]
        .spacing(20)
        .align_y(Alignment::Center),
    )
    .padding(26)
    .width(Length::Fill)
    .style(|_| panel_style(HERO, Color::from_rgb8(43, 73, 91)));

    let current_profile = container(
        column![
            row![
                text("ВЫБРАННАЯ ПЛАТФОРМА").size(10).color(MUTED),
                Space::new().width(Length::Fill),
                status_tag("НЕ УСТАНОВЛЕНО"),
            ]
            .align_y(Alignment::Center),
            Space::new().height(12),
            text(selected.1).size(24).color(TEXT),
            text(selected.0).size(10).color(ACCENT),
            text(selected.2).size(12).color(MUTED),
            Space::new().height(12),
            row![
                accent_button("▶  Играть", Message::Play),
                quiet_button("Настроить", Message::CreateInstance),
            ]
            .spacing(10),
        ]
        .spacing(6),
    )
    .padding(20)
    .width(Length::Fill)
    .style(|_| panel_style(PANEL, LINE));

    let platforms = row![
        platform_card(
            "01",
            "JAVA EDITION",
            "Forge · Fabric · NeoForge · Quilt",
            PlatformChoice::Java,
            app.platform == PlatformChoice::Java,
        ),
        platform_card(
            "02",
            "HORIZON",
            "HMOD · HNMOD · Rust runtime",
            PlatformChoice::Horizon,
            app.platform == PlatformChoice::Horizon,
        ),
    ]
    .spacing(14)
    .width(Length::Fill);

    let activity = container(
        column![
            row![
                text("БЫСТРЫЙ ДОСТУП").size(10).color(MUTED),
                Space::new().width(Length::Fill),
                text("СЛЕДУЮЩИЕ ШАГИ").size(10).color(MUTED),
            ],
            rule::horizontal(1).style(|_| rule::Style {
                color: LINE,
                ..Default::default()
            }),
            row![
                shortcut("▦", "Создай сборку", "Отдельная папка для каждой игры."),
                shortcut("◇", "Найди моды", "Каталоги появятся после интеграции API."),
                shortcut("♙", "Добавь аккаунт", "Microsoft и локальные профили."),
            ]
            .spacing(18)
            .width(Length::Fill),
        ]
        .spacing(14),
    )
    .padding(20)
    .width(Length::Fill)
    .style(|_| panel_style(PANEL, LINE));

    let mut body = column![
        hero,
        row![
            current_profile,
            platform_overview(app),
        ]
        .spacing(14)
        .width(Length::Fill),
        platforms,
        activity,
    ]
    .spacing(18)
    .width(Length::Fill);

    if let Some(notice) = &app.notice {
        body = body.push(
            container(text(notice).size(12).color(TEXT))
                .padding(12)
                .width(Length::Fill)
                .style(|_| panel_style(ACCENT_DARK, LINE)),
        );
    }

    body.into()
}

fn platform_overview(app: &App) -> Element<'_, Message> {
    let (title, description) = match app.platform {
        PlatformChoice::Java => (
            "Java загрузчики",
            "Классический Minecraft с поддержкой Forge, NeoForge, Fabric и Quilt.",
        ),
        PlatformChoice::Horizon => (
            "Horizon моды",
            "Отдельная экосистема контентных .hmod и нативных .hnmod модов.",
        ),
    };

    container(
        column![
            text("ЭКОСИСТЕМА").size(10).color(MUTED),
            Space::new().height(10),
            text(title).size(17).color(TEXT),
            text(description).size(12).color(MUTED),
        ]
        .spacing(7),
    )
    .padding(20)
    .width(Length::Fill)
    .style(|_| panel_style(PANEL, LINE))
    .into()
}

fn platform_card(
    number: &'static str,
    title: &'static str,
    description: &'static str,
    platform: PlatformChoice,
    selected: bool,
) -> Element<'static, Message> {
    let bg = if selected { ACCENT_DARK } else { PANEL };
    let edge = if selected { ACCENT } else { LINE };

    button(
        row![
            container(text(number).size(11).color(ACCENT))
                .width(34)
                .height(34)
                .center(Length::Fill)
                .style(move |_| panel_style(bg, Color::TRANSPARENT)),
            column![
                text(title).size(12).color(TEXT),
                text(description).size(10).color(MUTED),
            ]
            .spacing(5),
            Space::new().width(Length::Fill),
            text(if selected { "●" } else { "○" })
                .size(14)
                .color(if selected { ACCENT } else { MUTED }),
        ]
        .align_y(Alignment::Center)
        .spacing(12),
    )
    .on_press(Message::SelectPlatform(platform))
    .padding(15)
    .width(Length::Fill)
    .style(move |theme, status| {
        let mut style = button::secondary(theme, status);
        style.background = Some(bg.into());
        style.text_color = TEXT;
        style.border = Border::default().rounded(12).width(1).color(edge);
        style
    })
    .into()
}

fn shortcut(
    icon: &'static str,
    title: &'static str,
    description: &'static str,
) -> Element<'static, Message> {
    column![
        text(icon).size(21).color(ACCENT),
        text(title).size(12).color(TEXT),
        text(description).size(10).color(MUTED),
    ]
    .spacing(6)
    .width(Length::Fill)
    .into()
}

fn empty_page(
    title: &'static str,
    description: &'static str,
    empty_title: &'static str,
    action_label: &'static str,
    action: Message,
) -> Element<'static, Message> {
    column![
        container(
            column![
                text("GLOWSTONECL").size(10).color(ACCENT),
                Space::new().height(10),
                text(title).size(27).color(TEXT),
                text(description).size(13).color(MUTED),
                Space::new().height(16),
                rule::horizontal(1).style(|_| rule::Style {
                    color: LINE,
                    ..Default::default()
                }),
                Space::new().height(14),
                text(empty_title).size(17).color(TEXT),
                text("Этот раздел пока готовится. Его содержимое появится по мере разработки лаунчера.")
                    .size(12)
                    .color(MUTED),
                Space::new().height(8),
                accent_button(action_label, action),
            ]
            .spacing(8),
        )
        .padding(26)
        .width(Length::Fill)
        .style(|_| panel_style(PANEL, LINE)),
    ]
    .width(Length::Fill)
    .into()
}

fn settings_page() -> Element<'static, Message> {
    #[cfg(target_os = "linux")]
    let backend_info = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        "Обнаружена Wayland-сессия. Нативный Wayland backend доступен."
    } else if std::env::var_os("DISPLAY").is_some() {
        "Обнаружена X11-сессия. X11 backend доступен."
    } else {
        "Графическая сессия не обнаружена в переменных окружения."
    };

    #[cfg(not(target_os = "linux"))]
    let backend_info = "Используется системный оконный backend.";

    column![
        settings_card("ОКОННАЯ СИСТЕМА", "Wayland и X11", backend_info),
        settings_card(
            "ИГРОВЫЕ ПЛАТФОРМЫ",
            "Java Edition и Horizon",
            "Каждая платформа будет иметь отдельные настройки запуска, хранения и модификаций.",
        ),
        settings_card(
            "АККАУНТЫ И БЕЗОПАСНОСТЬ",
            "Локальное хранение данных",
            "Токены Microsoft будут храниться в системном хранилище учётных данных.",
        ),
    ]
    .spacing(14)
    .into()
}

fn settings_card(
    eyebrow: &'static str,
    title: &'static str,
    description: &'static str,
) -> Element<'static, Message> {
    container(
        column![
            text(eyebrow).size(10).color(ACCENT),
            text(title).size(17).color(TEXT),
            text(description).size(12).color(MUTED),
        ]
        .spacing(8),
    )
    .padding(20)
    .width(Length::Fill)
    .style(|_| panel_style(PANEL, LINE))
    .into()
}

fn section_label(label: &'static str) -> Element<'static, Message> {
    container(text(label).size(9).color(MUTED))
        .padding([0, 10])
        .into()
}

fn nav_button(
    icon: &'static str,
    label: &'static str,
    page: Page,
    current: Page,
) -> Element<'static, Message> {
    let active = page == current;

    button(
        row![
            text(icon)
                .size(16)
                .color(if active { ACCENT } else { MUTED }),
            text(label)
                .size(12)
                .color(if active { TEXT } else { MUTED }),
        ]
        .spacing(13)
        .align_y(Alignment::Center),
    )
    .on_press(Message::Navigate(page))
    .width(Length::Fill)
    .padding([11, 12])
    .style(move |theme, status| {
        let mut style = button::text(theme, status);
        if active {
            style.background = Some(PANEL_HOVER.into());
        }
        style.text_color = TEXT;
        style.border = Border::default().rounded(9);
        style
    })
    .into()
}

fn accent_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(12))
        .on_press(message)
        .padding([11, 16])
        .style(|theme, status| {
            let mut style = button::primary(theme, status);
            style.background = Some(ACCENT.into());
            style.text_color = Color::from_rgb8(10, 22, 29);
            style.border = Border::default().rounded(9);
            style
        })
        .into()
}

fn quiet_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(12))
        .on_press(message)
        .padding([11, 16])
        .style(|theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(PANEL_HOVER.into());
            style.text_color = TEXT;
            style.border = Border::default().rounded(9).color(LINE);
            style
        })
        .into()
}

fn status_tag(label: &'static str) -> Element<'static, Message> {
    container(text(label).size(9).color(MUTED))
        .padding([6, 9])
        .style(|_| panel_style(PANEL_HOVER, LINE))
        .into()
}

fn panel_style(background: Color, border: Color) -> container::Style {
    container::Style {
        background: Some(background.into()),
        border: Border::default().rounded(13).width(1).color(border),
        text_color: Some(TEXT),
        ..Default::default()
    }
}
