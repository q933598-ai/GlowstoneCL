use iced::widget::{
    button, canvas, column, container, row, rule, scrollable, text, Space,
};
use iced::{
    Alignment, Border, Color, Element, Length, Rectangle, Shadow, Vector,
};

const BACKGROUND: Color = Color::from_rgb8(10, 15, 24);
const TEXT_PRIMARY: Color = Color::from_rgb8(236, 243, 250);
const TEXT_SECONDARY: Color = Color::from_rgb8(153, 170, 190);
const TEXT_MUTED: Color = Color::from_rgb8(105, 124, 147);
const CYAN: Color = Color::from_rgb8(115, 218, 230);
const CYAN_TINT: Color = Color::from_rgba(0.18, 0.62, 0.72, 0.18);
const PURPLE: Color = Color::from_rgb8(174, 153, 255);
const GLASS: Color = Color::from_rgba(0.075, 0.12, 0.19, 0.78);
const GLASS_LIGHT: Color = Color::from_rgba(0.12, 0.18, 0.27, 0.68);
const GLASS_DARK: Color = Color::from_rgba(0.045, 0.075, 0.12, 0.84);
const GLASS_EDGE: Color = Color::from_rgba(0.72, 0.87, 1.0, 0.13);
const GLASS_EDGE_BRIGHT: Color = Color::from_rgba(0.57, 0.85, 0.98, 0.3);
const DIVIDER: Color = Color::from_rgba(0.65, 0.78, 0.92, 0.11);

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
    Info(&'static str),
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
        Message::Info(message) => {
            app.notice = Some(message.to_owned());
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let shell = row![
        navigation_rail(app),
        column![
            top_bar(app.page),
            scrollable(page_content(app))
                .height(Length::Fill)
                .width(Length::Fill),
        ]
        .height(Length::Fill)
        .width(Length::Fill),
    ]
    .spacing(14)
    .height(Length::Fill);

    iced::widget::stack![
        canvas(GlowBackdrop)
            .width(Length::Fill)
            .height(Length::Fill),
        container(shell)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(12),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[derive(Debug, Clone, Copy)]
struct GlowBackdrop;

impl<Message> canvas::Program<Message> for GlowBackdrop {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let background = canvas::Path::rectangle(iced::Point::ORIGIN, bounds.size());
        frame.fill(&background, BACKGROUND);

        // Layered low-opacity circles create ambient light behind the glass panels.
        draw_glow(
            &mut frame,
            iced::Point::new(bounds.width * 0.77, bounds.height * 0.16),
            410.0,
            Color::from_rgb8(34, 151, 181),
        );
        draw_glow(
            &mut frame,
            iced::Point::new(bounds.width * 0.39, bounds.height * 0.92),
            360.0,
            Color::from_rgb8(93, 78, 176),
        );
        draw_glow(
            &mut frame,
            iced::Point::new(bounds.width * 0.97, bounds.height * 0.84),
            260.0,
            Color::from_rgb8(34, 115, 132),
        );

        vec![frame.into_geometry()]
    }
}

fn draw_glow(
    frame: &mut canvas::Frame,
    center: iced::Point,
    radius: f32,
    color: Color,
) {
    for layer in 0..20 {
        let amount = layer as f32 / 19.0;
        let current_radius = radius * (1.0 - amount * 0.92);
        let alpha = 0.0018 + (1.0 - amount).powf(2.0) * 0.008;
        let circle = canvas::Path::circle(center, current_radius);
        frame.fill(
            &circle,
            Color::from_rgba(color.r, color.g, color.b, alpha),
        );
    }
}

fn navigation_rail(app: &App) -> Element<'_, Message> {
    let brand = container(
        text("✦")
            .size(25)
            .color(CYAN),
    )
    .width(48)
    .height(48)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(|_| glass_style(CYAN_TINT, GLASS_EDGE_BRIGHT));

    let nav = column![
        rail_button("⌂", "Главная", Page::Home, app.page),
        rail_button("▦", "Сборки", Page::Instances, app.page),
        rail_button("◇", "Моды", Page::Catalog, app.page),
        rail_button("▤", "Модпаки", Page::Modpacks, app.page),
        rail_button("↓", "Загрузки", Page::Downloads, app.page),
    ]
    .spacing(8)
    .align_x(Alignment::Center);

    let bottom = column![
        rail_button("♙", "Аккаунт", Page::Accounts, app.page),
        rail_button("⚙", "Настройки", Page::Settings, app.page),
        container(
            text("U")
                .size(13)
                .color(CYAN),
        )
        .width(40)
        .height(40)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_| glass_style(GLASS_LIGHT, GLASS_EDGE)),
    ]
    .spacing(10)
    .align_x(Alignment::Center);

    container(
        column![
            brand,
            Space::new().height(24),
            nav,
            Space::new().height(Length::Fill),
            rule::horizontal(1).style(|_| rule::Style {
                color: DIVIDER,
                radius: iced::border::Radius::default(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }),
            Space::new().height(14),
            bottom,
        ]
        .align_x(Alignment::Center)
        .height(Length::Fill),
    )
    .width(76)
    .height(Length::Fill)
    .padding([16, 9])
    .style(|_| glass_style(GLASS_DARK, GLASS_EDGE))
    .into()
}

fn rail_button(
    icon: &'static str,
    label: &'static str,
    page: Page,
    current: Page,
) -> Element<'static, Message> {
    let active = page == current;
    let background = if active {
        CYAN_TINT
    } else {
        Color::TRANSPARENT
    };
    let border = if active {
        GLASS_EDGE_BRIGHT
    } else {
        Color::TRANSPARENT
    };
    let icon_color = if active { CYAN } else { TEXT_SECONDARY };
    let label_color = if active { TEXT_PRIMARY } else { TEXT_MUTED };

    button(
        column![
            text(icon).size(19).color(icon_color),
            text(label).size(9).color(label_color),
        ]
        .spacing(5)
        .align_x(Alignment::Center),
    )
    .on_press(Message::Navigate(page))
    .width(58)
    .padding([9, 3])
    .style(move |theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(background.into());
        style.text_color = label_color;
        style.border = Border::default().rounded(14).width(1).color(border);
        style
    })
    .into()
}

fn top_bar(page: Page) -> Element<'static, Message> {
    let (title, subtitle) = page_heading(page);

    container(
        row![
            column![
                text(title).size(17).color(TEXT_PRIMARY),
                text(subtitle).size(10).color(TEXT_MUTED),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            button(
                row![
                    text("⌕").size(18).color(TEXT_SECONDARY),
                    text("Поиск модов и сборок").size(11).color(TEXT_SECONDARY),
                    Space::new().width(24),
                    text("Ctrl K").size(9).color(TEXT_MUTED),
                ]
                .align_y(Alignment::Center)
                .spacing(9),
            )
            .on_press(Message::Navigate(Page::Catalog))
            .padding([8, 13])
            .style(|theme, status| {
                let mut style = button::secondary(theme, status);
                style.background = Some(GLASS_LIGHT.into());
                style.text_color = TEXT_SECONDARY;
                style.border = Border::default().rounded(12).width(1).color(GLASS_EDGE);
                style
            }),
            button(
                row![
                    text("●").size(8).color(CYAN),
                    text("Локальный профиль").size(10).color(TEXT_PRIMARY),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .on_press(Message::Navigate(Page::Accounts))
            .padding([9, 13])
            .style(|theme, status| {
                let mut style = button::text(theme, status);
                style.background = Some(Color::TRANSPARENT.into());
                style.text_color = TEXT_PRIMARY;
                style
            }),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .padding([12, 18])
    .style(|_| glass_style(GLASS, GLASS_EDGE))
    .into()
}

fn page_heading(page: Page) -> (&'static str, &'static str) {
    match page {
        Page::Home => ("Главная", "Твоя игровая библиотека"),
        Page::Instances => ("Мои сборки", "Все игровые экземпляры в одном месте"),
        Page::Catalog => ("Каталог модов", "Открывай новые возможности Minecraft"),
        Page::Modpacks => ("Модпаки", "Готовые сборки и собственные коллекции"),
        Page::Downloads => ("Загрузки", "Очередь задач и история операций"),
        Page::Accounts => ("Аккаунты", "Microsoft и локальные профили"),
        Page::Settings => ("Настройки", "Персонализация и параметры запуска"),
    }
}

fn page_content(app: &App) -> Element<'_, Message> {
    let page = match app.page {
        Page::Home => home_page(app),
        Page::Instances => instances_page(),
        Page::Catalog => catalog_page(),
        Page::Modpacks => modpacks_page(),
        Page::Downloads => downloads_page(),
        Page::Accounts => accounts_page(),
        Page::Settings => settings_page(),
    };

    let page = if let Some(notice) = &app.notice {
        column![
            page,
            container(text(notice).size(11).color(TEXT_PRIMARY))
                .padding([10, 14])
                .width(Length::Fill)
                .style(|_| glass_style(CYAN_TINT, GLASS_EDGE_BRIGHT)),
        ]
        .spacing(12)
        .into()
    } else {
        page
    };

    container(page)
        .width(Length::Fill)
        .padding([14, 18])
        .into()
}

fn home_page(app: &App) -> Element<'_, Message> {
    let platform_name = match app.platform {
        PlatformChoice::Java => "MINECRAFT JAVA EDITION",
        PlatformChoice::Horizon => "HORIZON · RUST EDITION",
    };
    let platform_subtitle = match app.platform {
        PlatformChoice::Java => "Vanilla, Forge, NeoForge, Fabric и Quilt",
        PlatformChoice::Horizon => "Нативные .hmod и .hnmod модификации",
    };

    let hero = container(
        row![
            column![
                row![
                    status_chip("GLOWSTONECL  /  EARLY ACCESS", CYAN_TINT, CYAN),
                ]
                .align_y(Alignment::Center),
                Space::new().height(10),
                text("Твой Minecraft.").size(31).color(TEXT_PRIMARY),
                text("Твои правила.").size(31).color(TEXT_PRIMARY),
                text("Одна библиотека для Java Edition и Horizon. Собирай свои игровые миры и управляй модами без лишней суеты.")
                    .size(12)
                    .color(TEXT_SECONDARY),
                Space::new().height(10),
                row![
                    primary_button("＋  Создать сборку", Message::Navigate(Page::Instances)),
                    tonal_button("Открыть каталог", Message::Navigate(Page::Catalog)),
                ]
                .spacing(10),
            ]
            .spacing(4)
            .width(Length::FillPortion(7)),
            container(
                column![
                    container(
                        text("✦")
                            .size(64)
                            .color(CYAN),
                    )
                    .width(126)
                    .height(126)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(|_| glass_style(CYAN_TINT, GLASS_EDGE_BRIGHT)),
                    text("GLOWSTONE").size(11).color(TEXT_PRIMARY),
                    text("UNIVERSAL GAME HUB").size(8).color(TEXT_MUTED),
                ]
                .align_x(Alignment::Center)
                .spacing(9),
            )
            .width(Length::FillPortion(3))
            .height(210)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_| glass_style(Color::from_rgba(0.08, 0.24, 0.31, 0.52), GLASS_EDGE)),
        ]
        .align_y(Alignment::Center)
        .spacing(20),
    )
    .padding(25)
    .width(Length::Fill)
    .style(|_| glass_style(GLASS, GLASS_EDGE_BRIGHT));

    let overview = row![
        metric_card("01", "Игровые платформы", "Java + Horizon", CYAN),
        metric_card("02", "Каталоги модов", "Modrinth + CurseForge", PURPLE),
        metric_card("03", "Активные задачи", "Пока нет загрузок", TEXT_SECONDARY),
    ]
    .spacing(12)
    .width(Length::Fill);

    let platforms = row![
        platform_card(
            "JAVA EDITION",
            "Классический Minecraft",
            "Vanilla · Forge · NeoForge · Fabric · Quilt",
            "J",
            PlatformChoice::Java,
            app.platform == PlatformChoice::Java,
            CYAN,
        ),
        platform_card(
            "HORIZON",
            "Minecraft на Rust",
            "Собственные контентные и native-моды",
            "H",
            PlatformChoice::Horizon,
            app.platform == PlatformChoice::Horizon,
            PURPLE,
        ),
    ]
    .spacing(12)
    .width(Length::Fill);

    let current = container(
        column![
            row![
                text("ВЫБРАННАЯ ПЛАТФОРМА").size(9).color(TEXT_MUTED),
                Space::new().width(Length::Fill),
                status_chip("НЕ УСТАНОВЛЕНА", GLASS_LIGHT, TEXT_SECONDARY),
            ]
            .align_y(Alignment::Center),
            Space::new().height(8),
            text(platform_name).size(17).color(TEXT_PRIMARY),
            text(platform_subtitle).size(10).color(TEXT_SECONDARY),
            Space::new().height(7),
            row![
                text("Профили появятся здесь после создания экземпляра.")
                    .size(10)
                    .color(TEXT_MUTED),
                Space::new().width(Length::Fill),
                text("→").size(16).color(CYAN),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(4),
    )
    .padding(16)
    .width(Length::FillPortion(6))
    .style(|_| glass_style(GLASS, GLASS_EDGE));

    let quick_links = container(
        column![
            text("БЫСТРЫЙ ДОСТУП").size(9).color(TEXT_MUTED),
            Space::new().height(4),
            row![
                quick_link("▦", "Сборки", "Создать и настроить", Page::Instances),
                quick_link("◇", "Моды", "Найти дополнения", Page::Catalog),
                quick_link("▤", "Модпаки", "Импортировать сборку", Page::Modpacks),
            ]
            .spacing(10)
            .width(Length::Fill),
        ]
        .spacing(8),
    )
    .padding(16)
    .width(Length::FillPortion(5))
    .style(|_| glass_style(GLASS, GLASS_EDGE));

    let mut content = column![
        hero,
        overview,
        section_header("ИГРОВЫЕ ПЛАТФОРМЫ", "Выбери среду для своих сборок"),
        platforms,
        row![current, quick_links]
            .spacing(12)
            .width(Length::Fill),
    ]
    .spacing(16)
    .width(Length::Fill);
    content.into()
}

fn metric_card(
    number: &'static str,
    title: &'static str,
    subtitle: &'static str,
    accent: Color,
) -> Element<'static, Message> {
    container(
        row![
            container(text(number).size(11).color(accent))
                .width(34)
                .height(34)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_| glass_style(Color::from_rgba(accent.r, accent.g, accent.b, 0.12), GLASS_EDGE)),
            column![
                text(title).size(10).color(TEXT_PRIMARY),
                text(subtitle).size(9).color(TEXT_MUTED),
            ]
            .spacing(3),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding(13)
    .width(Length::Fill)
    .style(|_| glass_style(GLASS_LIGHT, GLASS_EDGE))
    .into()
}

fn platform_card(
    eyebrow: &'static str,
    title: &'static str,
    description: &'static str,
    mark: &'static str,
    platform: PlatformChoice,
    selected: bool,
    accent: Color,
) -> Element<'static, Message> {
    let background = if selected {
        Color::from_rgba(accent.r, accent.g, accent.b, 0.13)
    } else {
        GLASS
    };
    let edge = if selected { accent } else { GLASS_EDGE };

    button(
        row![
            container(text(mark).size(24).color(accent))
                .width(54)
                .height(54)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_| glass_style(
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.12),
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.25),
                )),
            column![
                text(eyebrow).size(9).color(accent),
                text(title).size(15).color(TEXT_PRIMARY),
                text(description).size(10).color(TEXT_SECONDARY),
            ]
            .spacing(5),
            Space::new().width(Length::Fill),
            text(if selected { "●" } else { "○" })
                .size(14)
                .color(if selected { accent } else { TEXT_MUTED }),
        ]
        .spacing(13)
        .align_y(Alignment::Center),
    )
    .on_press(Message::SelectPlatform(platform))
    .padding(16)
    .width(Length::Fill)
    .style(move |theme, status| {
        let mut style = button::secondary(theme, status);
        style.background = Some(background.into());
        style.text_color = TEXT_PRIMARY;
        style.border = Border::default().rounded(17).width(1).color(edge);
        style.shadow = Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
            offset: Vector::new(0.0, 5.0),
            blur_radius: 18.0,
        };
        style
    })
    .into()
}

fn quick_link(
    icon: &'static str,
    title: &'static str,
    subtitle: &'static str,
    page: Page,
) -> Element<'static, Message> {
    button(
        column![
            text(icon).size(19).color(CYAN),
            text(title).size(10).color(TEXT_PRIMARY),
            text(subtitle).size(8).color(TEXT_MUTED),
        ]
        .align_x(Alignment::Start)
        .spacing(6),
    )
    .on_press(Message::Navigate(page))
    .padding(11)
    .width(Length::Fill)
    .style(|theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(GLASS_LIGHT.into());
        style.text_color = TEXT_PRIMARY;
        style.border = Border::default().rounded(13).width(1).color(GLASS_EDGE);
        style
    })
    .into()
}

fn section_header(
    title: &'static str,
    subtitle: &'static str,
) -> Element<'static, Message> {
    row![
        column![
            text(title).size(9).color(CYAN),
            text(subtitle).size(11).color(TEXT_MUTED),
        ]
        .spacing(3),
        Space::new().width(Length::Fill),
        text(" ").size(8),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn instances_page() -> Element<'static, Message> {
    column![
        section_header("ТВОЯ БИБЛИОТЕКА", "Каждый профиль хранит собственные версии, моды и настройки."),
        container(
            column![
                text("▦").size(30).color(CYAN),
                text("Здесь пока тихо").size(18).color(TEXT_PRIMARY),
                text("Создай первый игровой экземпляр Minecraft Java или Horizon. Они будут храниться отдельно и не будут конфликтовать между собой.")
                    .size(11)
                    .color(TEXT_SECONDARY),
                Space::new().height(6),
                row![
                    primary_button("＋  Создать Java-сборку", Message::Info("Мастер создания Java-сборки будет реализован следующим этапом.")),
                    tonal_button("Создать Horizon-профиль", Message::Info("Мастер создания Horizon-профиля будет реализован следующим этапом.")),
                ]
                .spacing(10),
            ]
            .align_x(Alignment::Start)
            .spacing(10),
        )
        .padding(24)
        .width(Length::Fill)
        .style(|_| glass_style(GLASS, GLASS_EDGE)),
    ]
    .spacing(16)
    .into()
}

fn catalog_page() -> Element<'static, Message> {
    column![
        section_header("ИСТОЧНИКИ КОНТЕНТА", "Единый каталог будет учитывать платформу и версию игры."),
        row![
            source_card("MODRINTH", "Моды, модпаки, шейдеры и ресурспаки", "API ещё не подключён", CYAN),
            source_card("CURSEFORGE", "Большая библиотека модов и готовых сборок", "Требуется официальная API-интеграция", PURPLE),
        ]
        .spacing(12),
        container(
            column![
                text("Совместимость").size(14).color(TEXT_PRIMARY),
                text("GlowstoneCL будет показывать, подходит ли найденный контент для Java Edition или Horizon. Java JAR-моды не будут помечаться как совместимые с Horizon.")
                    .size(10)
                    .color(TEXT_SECONDARY),
            ]
            .spacing(8),
        )
        .padding(18)
        .width(Length::Fill)
        .style(|_| glass_style(GLASS, GLASS_EDGE)),
    ]
    .spacing(16)
    .into()
}

fn source_card(
    name: &'static str,
    description: &'static str,
    status: &'static str,
    accent: Color,
) -> Element<'static, Message> {
    container(
        column![
            text(name).size(12).color(accent),
            text(description).size(11).color(TEXT_PRIMARY),
            Space::new().height(8),
            status_chip(status, GLASS_LIGHT, TEXT_SECONDARY),
        ]
        .spacing(9),
    )
    .padding(18)
    .width(Length::Fill)
    .style(|_| glass_style(GLASS, GLASS_EDGE))
    .into()
}

fn modpacks_page() -> Element<'static, Message> {
    column![
        section_header("МОДПАКИ", "Импортируй готовые сборки или управляй собственными."),
        container(
            column![
                text("▤").size(29).color(PURPLE),
                text("Твоя следующая большая сборка начинается здесь").size(17).color(TEXT_PRIMARY),
                text("Планируется поддержка Modrinth .mrpack и CurseForge ZIP, а также экспорт конфигураций.")
                    .size(11)
                    .color(TEXT_SECONDARY),
                tonal_button("Импорт модпака", Message::Info("Импорт .mrpack и CurseForge ZIP пока не реализован.")),
            ]
            .spacing(10),
        )
        .padding(24)
        .width(Length::Fill)
        .style(|_| glass_style(GLASS, GLASS_EDGE)),
    ]
    .spacing(16)
    .into()
}

fn downloads_page() -> Element<'static, Message> {
    column![
        section_header("ОЧЕРЕДЬ ЗАДАЧ", "Загрузки игры, библиотек, модов и обновлений."),
        container(
            column![
                text("↓").size(30).color(CYAN),
                text("Нет активных загрузок").size(17).color(TEXT_PRIMARY),
                text("Когда начнёшь устанавливать игру или моды, здесь появятся прогресс, скорость и состояние проверки файлов.")
                    .size(11)
                    .color(TEXT_SECONDARY),
            ]
            .spacing(9),
        )
        .padding(24)
        .width(Length::Fill)
        .style(|_| glass_style(GLASS, GLASS_EDGE)),
    ]
    .spacing(16)
    .into()
}

fn accounts_page() -> Element<'static, Message> {
    column![
        section_header("АККАУНТЫ", "Управляй игровыми профилями из одного места."),
        row![
            account_card(
                "MICROSOFT",
                "Официальный аккаунт",
                "Для лицензированной Java Edition и Horizon.",
                CYAN,
            ),
            account_card(
                "OFFLINE",
                "Локальный профиль",
                "Для локального запуска Java и серверов без online-mode.",
                PURPLE,
            ),
        ]
        .spacing(12),
    ]
    .spacing(16)
    .into()
}

fn account_card(
    eyebrow: &'static str,
    title: &'static str,
    description: &'static str,
    accent: Color,
) -> Element<'static, Message> {
    container(
        column![
            text(eyebrow).size(9).color(accent),
            text(title).size(15).color(TEXT_PRIMARY),
            text(description).size(10).color(TEXT_SECONDARY),
            Space::new().height(8),
            status_chip("ЕЩЁ НЕ ДОБАВЛЕН", GLASS_LIGHT, TEXT_MUTED),
        ]
        .spacing(8),
    )
    .padding(18)
    .width(Length::Fill)
    .style(|_| glass_style(GLASS, GLASS_EDGE))
    .into()
}

fn settings_page() -> Element<'static, Message> {
    #[cfg(target_os = "linux")]
    let backend_info = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        "Обнаружена Wayland-сессия. Для Niri доступен нативный Wayland backend."
    } else if std::env::var_os("DISPLAY").is_some() {
        "Обнаружена X11-сессия. Доступен X11 backend."
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
            "Для каждой платформы будут отдельные настройки запуска, хранения и модификаций.",
        ),
        settings_card(
            "ВНЕШНИЙ ВИД",
            "Liquid glass + Material",
            "Полупрозрачные поверхности, тональные кнопки, мягкие акценты и компактная навигация.",
        ),
        settings_card(
            "БЕЗОПАСНОСТЬ",
            "Локальное хранение данных",
            "Токены Microsoft будут храниться в системном хранилище учётных данных.",
        ),
    ]
    .spacing(12)
    .into()
}

fn settings_card(
    eyebrow: &'static str,
    title: &'static str,
    description: &'static str,
) -> Element<'static, Message> {
    container(
        column![
            text(eyebrow).size(9).color(CYAN),
            text(title).size(15).color(TEXT_PRIMARY),
            text(description).size(10).color(TEXT_SECONDARY),
        ]
        .spacing(8),
    )
    .padding(18)
    .width(Length::Fill)
    .style(|_| glass_style(GLASS, GLASS_EDGE))
    .into()
}

fn status_chip(
    label: &'static str,
    background: Color,
    foreground: Color,
) -> Element<'static, Message> {
    container(text(label).size(8).color(foreground))
        .padding([6, 9])
        .style(move |_| glass_style(background, GLASS_EDGE))
        .into()
}

fn primary_button(
    label: &'static str,
    message: Message,
) -> Element<'static, Message> {
    button(text(label).size(11))
        .on_press(message)
        .padding([11, 16])
        .style(|theme, status| {
            let mut style = button::primary(theme, status);
            style.background = Some(CYAN.into());
            style.text_color = Color::from_rgb8(9, 22, 30);
            style.border = Border::default().rounded(12).width(1).color(CYAN);
            style.shadow = Shadow {
                color: Color::from_rgba(0.1, 0.75, 0.9, 0.18),
                offset: Vector::new(0.0, 4.0),
                blur_radius: 14.0,
            };
            style
        })
        .into()
}

fn tonal_button(
    label: &'static str,
    message: Message,
) -> Element<'static, Message> {
    button(text(label).size(11))
        .on_press(message)
        .padding([11, 16])
        .style(|theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(CYAN_TINT.into());
            style.text_color = TEXT_PRIMARY;
            style.border = Border::default().rounded(12).width(1).color(GLASS_EDGE_BRIGHT);
            style
        })
        .into()
}

fn glass_style(background: Color, edge: Color) -> container::Style {
    container::Style {
        background: Some(background.into()),
        border: Border::default().rounded(18).width(1).color(edge),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}
