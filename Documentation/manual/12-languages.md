# 12. Languages

A game is written in one language and can offer translations. Players choose the language in
*Options › Language*; by default the game starts in the system's (in a browser: the browser's)
language when the game offers it.

## Translating a game

1. *Database › Project › Languages*: set the language the game is **written in** (a code like `en`).
2. **Export translation template** with a language code (`de`, `fr`, `pt-br`…). It writes
   `lang/<code>.ron` into the project with every text players see, each with an empty translation:
   names and descriptions of items, props, characters, quests and their stages, skills, spells and
   achievements, the game's name, the lines and choices of every trigger, the lock and container
   messages, and the texts scripts pass to `say` and `choice`.
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
   An empty or missing translation shows the original text.
4. After changing the game, **Update template** adds new texts, keeps every translation already
   made, and reports texts no longer used (they stay until you delete them). The list shows how many
   texts each language has translated.
5. **Play-test in** starts play-tests in a chosen language.

Exported games carry the translation files.

## The engine's own texts

R(ust)PG2D's menus, windows, HUD and messages come translated for **German**. A project's
`lang/<code>.ron` can add or replace them in a `ui` section, where `{}` stands for a value:

```
ui: {
    "Lv {}": "St. {}",
}
```

The game's font covers Latin-1 (ä, ö, ü, ß, é, ñ…), not other scripts.

The example project offers German for all of its texts.
