# Производство графики и инструкция для следующей сессии

[Стиль](README.md) · [Ассеты](ASSETS.md) · [Референс](references/wildforge-direction-v1.png)

## Как продолжить

Прочитать все три документа и открыть референс. Проверить текущий код, содержимое content и git diff. Последний референс имеет приоритет перед ранними пиксельными концептами из чата.

Начать с P0. Первым набором подготовить stone_blocks, road, wood, cloth и metal, затем лист героя и волка. Проверить материалы на простых моделях с рабочей камерой. После этого делать UV-атласы, модели и анимации.

Все размеры и бюджеты — исходное предложение для проверки. По результатам менять документ вместе с ассетом, чтобы следующая генерация использовала актуальные параметры.

## Что даёт генерация

Imagegen создаёт концепты, изображения поверхностей, иконки и изображения по заданной UV-развёртке. Результат требует проверки бесшовности, размеров, альфы и соответствия модели. Согласованность нескольких ракурсов также проверяется вручную.

Готовая игровая 3D-модель требует отдельного этапа моделирования, UV, скелета, весов и анимаций. Для этого применить доступный 3D-инструмент, например Blender через его API, либо внешний генератор 3D с последующей очисткой. Наличие такого инструмента проверить в начале работы. В текущем проекте модели создаются процедурно из Rust.

## Файлы и версии

Предлагаемая структура для будущих результатов

```
docs/art/references/                 утверждённые визуальные ориентиры
art/source/textures/<id>/v001/       исходная генерация, prompt.txt, заметки
art/source/models/<id>/v001/         ракурсы, исходник моделирования, UV
art/source/ui/<id>/v001/             исходные элементы интерфейса
content/textures/                   проверенные игровые PNG
client/godot/assets/models/         импортируемые GLB
client/godot/assets/ui/              импортируемые UI PNG
```

Создавать каталоги по мере производства. Сохранять новую версию рядом с предыдущей. Для принятого ассета записывать ID, версию, исходный промпт, использованные референсы, размер, способ обработки, статус и результаты проверки. Статусы — draft, reviewed, integrated.

Генерировать через встроенный imagegen. Если навык imagegen изменился, прочитать его актуальную версию. Рабочие ассеты копировать в репозиторий. Для текстур и UI итоговый размер проверять после генерации. Преобразование размера и формата выполнять инструментом обработки изображений, сохраняя исходник. Художественные правки отправлять в imagegen.

## Общий блок промпта

Добавлять этот блок к каждому запросу. Эталон прикладывать как reference image.

```text
Original Wildforge dark fantasy PC RPG art direction, late 1990s to early
2000s. Natural muted colors, aged grey stone, dark worn leather, faded
ochre cloth, rusty iron, cold blue-grey atmospheric distance and restrained
warm torchlight. Strong angular low-polygon silhouettes, broad visible
planar faces, grounded proportions, purposeful asymmetry and signs of use.
Moderately detailed diffuse textures, coherent readable material patterns.
Fine texture detail at normal screen resolution. Match the attached
Wildforge direction reference in palette and material character.
Avoid oversized mosaic pixels, chibi proportions, cartoon outlines,
glossy modern mobile fantasy, photographic microdetail and cinematic blur.
```

Уточнения конкретного типа ассета ниже определяют фон, свет и композицию.

## Бесшовная текстура

Поля подставляются из ASSETS.md. Один запрос создаёт одну поверхность.

```text
Asset: {ID}, seamless diffuse/albedo texture.
Material: {material description}.
Color: {base color and allowed variations}.
Orthographic straight-on surface view, filling the entire square.
Even neutral diffuse illumination. Surface color and restrained crevice
variation only. No directional cast shadows or baked specular highlights.
Large-scale readable material structure, sparse medium detail.
Seamless on both horizontal and vertical edges. Intended world coverage
{meters} metres, final game texture {width} by {height} pixels.
No frame, labels, perspective, objects on the surface or mockup presentation.
Deliver a single flat texture suitable for repeat mapping.
```

Пример stone_blocks

```text
Asset: stone_blocks. Seamless diffuse/albedo texture of worn grey limestone
masonry, uneven large rectangular blocks, narrow dark joints, a few chipped
corners, sparse desaturated olive moss in cracks. Base grey #77746B,
lighter wear #AAA18A. One tile covers about 2 by 2 metres, with 4–6 stones
across. Even neutral lighting, straight-on orthographic surface.
Seamless both axes. Final game texture 256 by 256 pixels.
Broad readable stone forms with restrained cracks. Single flat texture.
```

Проверка — повтор 3×3, отсутствие заметных границ, оценка повторяющихся пятен. При необходимости запросить исправление конкретного шва. После приведения к итоговому размеру повторить проверку на игровой плоскости.

## Референс модели

```text
Asset: {ID}, consistent character/model turnaround for 3D modeling.
Same design in front, right side and back orthographic views, aligned
ground line and identical scale, plus one separate three-quarter view.
Neutral mid-grey background, even studio light, full silhouette visible.
{Anatomy, clothing, materials and signature features from ASSETS.md}.
Clearly visible low-poly planar construction and angular silhouette.
Neutral relaxed A-pose for humanoids, legs separated for quadrupeds.
Equipment shown separately at matching scale. No perspective distortion.
Fine painted texture details, no exaggerated texture pixel blocks.
Maintain identical proportions, seams, straps and accessories in all views.
```

Для героя явно указать рост 1.8 м, охристый плащ, один железный наплечник, тёмные ремни и сапоги. Положение наплечника закрепить по принятому листу. Перед моделированием сопоставить длины конечностей и положение деталей во всех ракурсах.

## Текстура UV-атласа

Сначала получить реальную UV-развёртку модели и подписанную копию с назначением островов. Для генерации приложить UV guide и лист внешнего вида как два отдельных референса.

```text
Create the diffuse color atlas for this exact UV layout.
Image 1 is the UV guide with fixed island positions.
Image 2 is the approved character appearance reference.
Preserve all island positions, sizes and orientations.
Paint {materials} inside their corresponding islands.
Neutral diffuse illumination, fabric folds and restrained material wear.
Keep face features aligned to the face UV island.
Provide edge color padding of {padding} pixels at final resolution.
Final atlas {size}. Single flat atlas, no labels or UV guide lines.
```

Padding начать с 8 px для 512² и 4 px для 256². Проверить результат на модели со всех сторон. Генерация по UV может потребовать ручной коррекции или texture painting. Особенно проверить лицо, зеркальные острова и швы.

## Иконка

```text
One inventory icon of {item}, matching the Wildforge reference.
Three-quarter view, full silhouette, centered with 6-pixel safe margins
at the final 64 by 64 size. Worn practical materials, clear shape,
restrained upper-left lighting. Transparent background.
No frame, text, numbers or surrounding objects.
```

Передавать transparent_background=true. Проверить читаемость при 32×32 и 64×64 на светлой и тёмной подложках.

## Промпт воспроизведения основного концепта

Использовать для нового кадра или сравнения направления. Текущий эталон был создан встроенным imagegen последовательными правками.

```text
Original Wildforge third-person dark fantasy RPG gameplay concept.
Late-1990s / early-2000s textured low-poly PC graphics. Full-body adventurer
seen from behind, wearing an ochre patched cloak, asymmetrical iron
shoulder armor and dark boots, plain sword. Clearly angular elbows,
wedge boots, pointed polygon hair, broad triangular folds in the cloak.
Ancient angular stone gateway in a worn frontier settlement, plaster
houses, timber awnings, pottery and crates, lean faceted wolf near ruins,
distant fortress and jagged mountains in cool fog. Natural grey limestone,
brown earth, faded red banners and olive scrub. Warm torches.
Fine modest-resolution diffuse textures, normal clear image resolution,
visible planar geometry, restrained shading. Compact dark iron RPG HUD,
health top left, map top right, nine item slots below.
Landscape composition. Avoid oversized square texels and smooth modern
high-poly rendering. Match the attached Wildforge direction reference.
```

## Требования к готовой модели

- Единицы — метры, Y вверх, перед персонажа +Z, как в текущем клиенте.
- Origin персонажа и ресурса на земле в центре основания.
- Применённые трансформации, scale 1, осмысленные имена узлов.
- Формат обмена GLB, исходник моделирования сохранён рядом с референсами.
- Деформация проверена после импорта в Godot.
- Слоты WeaponSocket и ArmorSocket согласованы с рукой и торсом.
- Анимации Idle, Walk, Attack, Hit, Death; Gather для героя.
- Перемещение персонажа задаётся существующим состоянием сервера. Анимации in-place.
- Коллизии простые, отдельно от декоративной геометрии.
- Alpha cutout для листвы, глубина и тени проверены при движении камеры.
- UV без случайных пересечений. Осознанное зеркалирование допустимо для повторяющихся деталей.

## Интеграция с текущим кодом

Сейчас client/rust/src/world/models.rs возвращает ActorModel, содержащий root,
body, weapon_pivot, weapon, height и legs. entities.rs управляет материалом
торса, видимостью оружия, поворотом и анимацией ног.

Импорт GLB потребует адаптации ActorModel и управления AnimationPlayer.
Сохранить отображение экипировки, положение HP/имени, масштаб и освобождение
нод при исчезновении сущности. Проверить снятие и смену оружия и брони.

content.rs загружает PNG и умножает цвет на tint. Новые атласы требуют белого
tint и отдельных материалов. Кэш материалов должен учитывать новые параметры,
если они станут настраиваемыми.

resources.rs использует MultiMesh для массовых ресурсов. Подготовить меши
и материалы импортированных ресурсов для того же механизма, контролируя
число частей и draw calls. Положение и размер должны соответствовать
серверному тайлу и блокировке прохода.

Новые дома и арки пока являются планом сцены. Прежде чем сделать их игровыми
препятствиями, согласовать размещение с серверным миром и коллизиями.

Существующий server/cmd/texgen пишет в content/textures. Перед его запуском
проверять список выходных файлов, чтобы сохранить принятые авторские текстуры.

## Проверка результата

1. Сборка клиента и импорт ресурсов.
2. Осмотр модели на нейтральном фоне спереди, сбоку и сзади.
3. Проверка UV, нормалей и альфы на близкой дистанции.
4. Просмотр в игре при штатной камере и разном зуме.
5. Проверка анимаций, экипировки и нескольких экземпляров.
6. Проверка тёплого и холодного освещения.
7. Сравнение скриншота с эталоном; запись отличий и решения в notes.md ассета.
8. Замер производительности в плотной сцене.
9. Только после визуального осмотра отмечать ассет как reviewed, после подключения и проверки как integrated.
