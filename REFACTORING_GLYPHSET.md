Technical Specification: Semantic VRAM & Global Glyph Registry
🏛 Часть 1. Архитектурное видение (Architecture Overview)

Текущая реализация текстовых буферов в VoidGrid хранит физические индексы тайлов атласа (glyph: u32) прямо в ячейках буфера. Это ограничивает движок: нельзя на лету менять шрифты для уже написанного текста, а поиск вариантов (bold, inverted) вызывает аллокации строк на этапе рендера.

Цель рефакторинга: Перевести архитектуру буферов на семантическую модель VRAM (как в классических ретро-компьютерах). Буфер должен хранить только семантический смысл (какой это символ и в каком он состоянии), а перевод смысла в пиксели должен происходить в рендерере через O(1) массивы (LUT - Lookup Tables) без единой аллокации.
Три кита новой архитектуры:
1. Global Glyph Registry (Источник истины)

Глобальный менеджер всех загруженных физических ресурсов (.png и .json).

    Монтирование: Принимает конфигурации и монтирует их в виртуальное дерево неймспейсов (например, assets/petscii.json монтируется в префикс characters/petscii/c64).

    Сквозной ID: Каждому физическому тайлу присваивается уникальный global_id: u32.

    Плоское хранилище: Менеджер хранит плоский массив entries: Vec<(AtlasKey, u32)>, где индекс — это global_id. Доступ к физическим координатам по global_id происходит за O(1).

    Кэш путей: Дерево путей разворачивается в плоский кэш HashMap<String, u32> для быстрого поиска global_id по строке (с учетом заранее разрешенных fallback-значений).

2. Glyphset (Логический маппер буфера)

Набор правил, который привязывается к конкретному Buffer. Он определяет, как семантические коды превращаются в global_id.

    Строгость: Гарантирует, что все используемые в нем глифы имеют одинаковый физический размер (tile_w, tile_h).

    Pre-baking (Прекомпиляция): При создании Glyphset опрашивает GlobalGlyphRegistry и строит внутри себя плоские массивы (LUT).

    Мгновенный доступ: В рантайме Glyphset не ищет строки и не разрешает фоллбэки. Он просто отдает global_id из двумерного массива по индексам [variant_id][code].

3. Семантический Character (VRAM)

Ячейка буфера больше не знает про атласы.

    code: u32 — семантический код (например, U+0048 для 'H' или любой кастомный байт).

    variant_id: u8 — ID состояния (0 = default, 1 = bold, 2 = inverted и т.д.).

Пайплайн данных:

    Текст: write_string("A", variant: "bold") -> Узнает, что bold это variant_id = 1 -> Пишет в буфер Character { code: 65, variant_id: 1 }.

    UI/Графика: Запрашивает у Glyphset: «Дай код для ui/borders:corner» -> Получает семантический код 218 -> Пишет в буфер Character { code: 218, variant_id: 0 }.

    Рендер: Читает из буфера code=65, variant=1 -> glyphset.luts[1][65] отдает global_id=4050 -> registry.entries[4050] отдает (AtlasKey, 28) -> Рендерер рисует 28-й тайл нужного атласа.

🛠 Часть 2. Структуры данных (Data Structures Blueprint)

Внедрить следующие структуры (или обновить существующие):
Rust

// 1. types.rs
new_key_type! {
    pub struct AtlasKey;
    pub struct BufferKey;
    pub struct PaletteKey;
    pub struct ShaderKey;
    pub struct GlyphsetKey; // НОВОЕ
}

pub struct Character {
    pub code: u32,
    pub variant_id: u8,
    pub fcolor: Color,
    pub bcolor: Color,
    pub fg_blend: Blend,
    pub bg_blend: Blend,
    pub transform: Transform,
    pub mask: Option<Mask>, // Mask остается привязанной к AtlasKey/glyph (физике)
}

// 2. global_registry.rs (НОВЫЙ МОДУЛЬ)
pub struct GlobalGlyphRegistry {
    /// O(1) доступ к физике. Индекс массива = global_id
    entries: Vec<(AtlasKey, u32)>, 
    
    /// Плоский кэш путей для поиска (уже с учетом fallback-логики дерева)
    path_cache: HashMap<String, u32>,
    
    // (Опционально) Внутреннее дерево для сложного монтирования и генерации path_cache
    // root: NamespaceNode,
}

// 3. glyphset.rs (НОВЫЙ МОДУЛЬ)
pub struct Glyphset {
    pub name: String,
    pub tile_w: u32,
    pub tile_h: u32,
    
    /// Маппинг имен вариантов в их ID (например, "bold" -> 1)
    pub variant_names: HashMap<String, u8>,
    
    /// LUT для O(1) рендеринга. 
    /// Индекс внешнего вектора - variant_id.
    /// Индекс внутреннего вектора - code (до 0xFFFF или разумного лимита).
    /// Значение - global_id из GlobalGlyphRegistry.
    pub luts: Vec<Vec<u32>>,
    
    /// Дополнительный словарь для UI ("ui/borders:top_left" -> code)
    pub namespace_map: HashMap<String, u32>,
    
    pub default_global_id: u32,
}

📋 Часть 3. Пошаговый план реализации (Implementation Steps)
Step 1: Update Core Types (src/types.rs)

    Добавь GlyphsetKey.

    Перепиши Character на code и variant_id. Удали glyph и variant: Option<String>.

    Обнови конструкторы Character::new, Character::full, Character::blank. В blank передавай default_code: u32 (например, пробел 32).

Step 2: Implement GlobalGlyphRegistry

    Создай структуру GlobalGlyphRegistry. Добавь её инстанс в Grids.

    Реализуй метод register_glyph(&mut self, atlas: AtlasKey, local_glyph: u32) -> u32, который пушит данные в entries и возвращает global_id (размер вектора - 1).

    Реализуй метод добавления путей: map_path(&mut self, path: String, global_id: u32). Он должен писать в path_cache.

    Реализуй метод query(&self, path: &str) -> Option<u32>.

Step 3: Implement Glyphset and LUT Pre-baking

    Создай Glyphset.

    Напиши логику его сборки. Сборщик должен получать на вход конфигурацию маппингов и ссылку на GlobalGlyphRegistry.

    Правило LUT: Ограничь внутренние массивы luts размером 65536 (или 1114112 для full unicode, если памяти не жалко, но лучше BMP).

    Fallback Resolution: При сборке Glyphset, если для какого-то variant не указан маппинг символа, в этот индекс LUT должен скопироваться global_id из базового LUT (где variant_id = 0). Рендерер не должен проверять фоллбэки!

Step 4: Update Buffers & Grids (src/buffer.rs, src/grids.rs)

    В Buffer замени pub(crate) atlas: AtlasKey на pub(crate) glyphset: GlyphsetKey.

    Замени поле default_variant: Option<String> на default_variant_id: u8.

    В Grids обнови методы создания буферов, чтобы они принимали GlyphsetKey.

    Добавь pub fn set_buffer_glyphset(&mut self, buffer: BufferKey, glyphset: GlyphsetKey), чтобы поддерживать динамическую смену шрифтов на лету.

Step 5: Rewrite Text Operations (src/text_ops.rs)

    write_string больше не мапит символы в физические тайлы.

    Для каждого char в строке он просто делает каст: code = ch as u32.

    Для разрешения variant: &str в variant_id: u8 функция должна один раз запросить ID у Glyphset перед циклом записи.

    Функция записывает Character { code, variant_id } в буфер. Это O(1) и zero-allocation.

Step 6: Update Renderer (src/renderer.rs)

    В draw_single_buffer и collect_shader_buffers:

        Получи Glyphset буфера.

        Возьми tile_w и tile_h прямо из Glyphset.

        Для каждого Character извлеки global_id:
        let global_id = glyphset.luts[ch.variant_id as usize][ch.code as usize];

        Получи физику из реестра:
        let (atlas_key, physical_glyph) = grids.global_registry.entries[global_id as usize];

        Отрисуй тайл.

🛑 Часть 4. Sanity Checks & Strict Rules

    Zero Allocations in Render: В модуле renderer.rs внутри циклов x..w и y..h ЗАПРЕЩЕНО использовать .to_string(), .clone() для строк, аллоцировать векторы или обращаться к HashMap. Только чтение из векторов (LUT) по индексу.

    Tile Size Consistency: При добавлении данных в Glyphset, если физический размер тайла в атласе не совпадает с glyphset.tile_w / tile_h, сборка должна паниковать или возвращать Err. Буфер не может смешивать размеры сеток.

    Graceful Fallback: Если ch.code выходит за пределы размера вектора LUT (например, кто-то передал эмодзи, а LUT расчитан на 256 символов ASCII), рендерер должен использовать безопасный метод (например, .get().unwrap_or(default_global_id)), чтобы не получить Index out of bounds панику.

*** (Конец файла REFACTORING_GLYPHSET.md)