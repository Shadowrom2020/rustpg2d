# 12. Languages

A game is written in one language and can offer translations. Players choose the language in
*Options › Gameplay › Language*; by default the game starts in the system's (in a browser: the browser's)
language when the game offers it.

## Translating a game

1. *Database › Project › Languages*: set the language the game is **written in** (a code like `en`).
2. **Export translation template** with a language code (`de`, `fr`, `pt-br`…). It writes
   `lang/<code>.ron` into the project with every text players see, each with an empty translation:
   names and descriptions of items, props, characters, quests and their stages, skills, spells and
   achievements, the game's name, the maps' titles, the lines and choices of every trigger, the
   lock, container and furniture messages, and the texts scripts pass to `say` and `choice`.
   A script text is found only as one string: write `say("You have {coins} coins.")` after
   `set_var("coins", item_count("coin"))`, not `say("You have " + n + " coins.")`, whose pieces
   can't be translated.
3. Fill in the translations with any text editor:

   ```
   (
       name: "Deutsch",
       texts: {
           "Apple": "Apfel",
           "Welcome to Emberbrook, {player}.": "Willkommen in Glutbach, {player}.",
           "Halt, traveller!": "",
       },
   )
   ```

   Keep the placeholders (`{player}`, `{variable}`); `{sir|madam}` gets the pair of the new language.
   Keep a dialog's [message codes](07-events.md#message-codes) too, around the words they belong to
   in the new language (`{c:gold}Schlüssel{/c}`); *Problems* lists translations that lost one.
   An empty or missing translation shows the original text.
4. After changing the game, **Update template** adds new texts, keeps every translation already
   made, and reports texts no longer used (they stay until you delete them). The list shows how many
   texts each language has translated.
5. **Play-test in** starts play-tests in a chosen language.

Exported games carry the translation files.

## The engine's own texts

R(ust)PG2D's menus, windows, HUD and messages come translated for **German** (`de`), **French**
(`fr`), **Spanish** (`es`), **Brazilian Portuguese** (`pt-br`, also used for `pt`), **Italian**
(`it`), **Polish** (`pl`), **Russian** (`ru`), **Japanese** (`ja`) and **Simplified Chinese**
(`zh-hans`, also used for `zh` and `zh-cn`). A project that offers one of these languages only
translates its own content. All but German are drafts made with AI help: corrections from native
speakers are welcome (the files are `crates/engine/lang/<code>.ron` in the repository).

Polish, Russian, Japanese and Chinese need a project font with their letters (below): monogram,
the example's font, covers Polish and Russian; the starter library's Fusion Pixel fonts cover
Japanese and Chinese. *Edit › Problems* checks the built-in texts of each language the project
offers along with its own.

A project's `lang/<code>.ron` can add or replace the engine's texts in a `ui` section, where `{}`
stands for a value (`{0}`, `{1}` pick values by position, to change their order):

```
ui: {
    "Lv {}": "St. {}",
}
```

## Fonts for other scripts

The built-in font covers Latin-1 (ä, ö, ü, ß, é, ñ…), not other scripts. For Russian, Greek,
Japanese, Chinese, Korean and the like, add a font that has them ([Database › Fonts](05-database.md#fonts))
and choose it under *Database › Project › Fonts*, as the font of a role or as a **fallback**: each
character comes from the first font that has it (the role's font, then the fallbacks, then the
built-in font), so one line can mix a pixel font's Latin letters with a CJK font's characters.
*Edit › Problems* lists the characters of each language that no font covers. Fusion Pixel Font,
Ark Pixel Font (both SIL Open Font License) and the Noto fonts cover Chinese, Japanese and Korean.

Lines wrap at spaces and, in Chinese and Japanese, between characters, following the usual rules
(*kinsoku*): a line never starts with a closing mark (、。」！？ ー, small kana) and never ends
with an opening one (「（). Korean wraps at its spaces. Lines in other scripts that use no spaces
(Thai, for one) still need line breaks (`\n`) where a line should end; right-to-left scripts
(Arabic, Hebrew) aren't supported yet.

The *Language* option shows each language by its own name (*Deutsch*, *日本語*, *Русский*),
from the `name` of its `lang/<code>.ron` or a built-in list. Players can type names in any
script the fonts have (the character creator), but not through an input method (IME): letters
typed directly arrive, composed Japanese or Chinese input does not.

The editor draws its forms with its own font, plus the fonts the project uses as fallbacks: once a
Japanese font is a fallback under *Project › Fonts*, the editor shows Japanese text too.

The example project offers German for all of its texts.
