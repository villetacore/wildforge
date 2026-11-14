# content/

Игровой контент Wildforge: только данные, без кода. Сервер загружает и проверяет
эти файлы при старте (все ссылки между ними) и отправляет клиенту нужные определения
при входе. Клиент берёт отсюда текстуры.

| Файл | Что описывает |
|---|---|
| `items.json` | шаблонные предметы. `type`: `material`, `potion` (`heal`, `mana`, `cleanse`, `cure`), `food` (`mood`, `status`, `heal`, `pet_heal`), `weapon` (`attack`), `armor` (`defense`), `tool` (`tool` + `tier`), `tome` (`spell`), `pet` (`pet`). `price` — базовая цена, `stack`, `color` |
| `recipes.json` | рецепты верстака (C): `result`, `qty`, `ingredients` |
| `resources.json` | добываемые узлы: `item`, `tool`, `min_tier`, `charges`, `respawn`, модель (`tree`, `pine`, `palm`, `deadtree`, `herb`, `rock`, `ore`, `crystal`), `spawn` (тип земли и шанс), необязательный `zones` |
| `bestiary.json` | **враги собираются из частей**: `families` (модель, статы и рост, дальняя атака `range`/`bolt`, допустимые стихии, дроп, `tameable`, `faction`), `elements` (имя, цвет свечения, статус при ударе, множители, дроп эссенции), `ranks` (множители и число черт), `traits` (модификаторы), `boss_names` |
| `gear.json` | **снаряжение собирается из частей**: `bases` (слот, модель, тип материала, статы, сколько материала на ковку), `materials`, `qualities`, `rarities` (число аффиксов), `affixes` (префиксы/суффиксы со статами и слотами), `enchants` (статы, слоты, реагенты, золото), `legend_names` |
| `magic.json` | `spells` (школа, вид: `bolt`, `dot`, `aoe_target`, `aoe_self`, `chain`, `heal`, `buff`, `drain`; мана, кулдаун, сила и масштаб от силы заклинаний, статус) и `statuses` (длительность, урон/лечение во времени, замедление, оглушение, щит, модификаторы статов, болезни) |
| `perks.json` | ветки и перки: `tier` (1–4), `stats` — бонусы, ключи описаны в `server/internal/game/stats.go` |
| `factions.json` | фракции с союзниками и врагами, ранги репутации и множители цен |
| `pets.json` | питомцы (модель, статы, полёт, способность) и способности |
| `zones.json` | зоны: центр, биом, уровни, цвета тумана и миникарты, город (стиль, фракция, радиус), семейства и веса стихий, подземелье (тема, семейства, уровень) |
| `npcs.json` | роли NPC: титул, одежда, маркер, имена, приветствие, ассортимент лавки; `rumors` для трактирщиков |
| `world.json` | размер мира, число врагов, слоты инвентаря, стартовые золото, заклинания и инвентарь |
| `textures/` | игровые PNG. 256×256 — расписанные атласы (`art/source/textures/retro-v1/`), 512×512 — процедурные (`snow`, `swamp`, `ash`, `lava`, `cobble`, `roof_tiles`, `slate`, `marble`, `sandstone`, `timber`, `moss`, `ice`, `steel`, `window`). `go run ./cmd/texgen` в `server/` пересоздаёт только процедурные; флаг `-legacy` перезапишет и атласные |

Типы земли для `spawn`: `grass`, `forest`, `sand`, `road`, `rocky`, `snow`, `swamp`, `ash`, `cobble`.

Ключи статов: `atk`, `def`, `hp`, `mana`, `str`, `dex`, `int`, `vit`, `crit`, `sp`,
`speed`, `lifesteal`, `regen`, `mana_regen`, `thorns`, `dodge`, `cdr`, `mf`, `gold`, `pet`,
стихийный урон `fire`, `frost`, `poison`, `shadow`, `storm`, `holy`, проценты `*_pct`.
