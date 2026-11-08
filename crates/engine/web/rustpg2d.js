// R(ust)PG2D's plugin for macroquad's JS loader (load it after mq_js_bundle.js, before
// load(...)): the browser storage and clock the web game uses (engine/src/sys.rs). Files the
// game writes (save games, settings, achievements) are kept in localStorage under
// "rustpg2d:<page folder>:<path>" as base64 text, so each game on a site has its own.
"use strict";
(function () {
    const prefix = "rustpg2d:" + location.pathname.replace(/[^/]*$/, "") + ":";
    const decoder = new TextDecoder();
    const encoder = new TextEncoder();
    const text = (ptr, len) => decoder.decode(new Uint8Array(wasm_memory.buffer, ptr, len));
    let pending = null;
    const take = (dest) => {
        new Uint8Array(wasm_memory.buffer, dest, pending.length).set(pending);
        pending = null;
    };
    miniquad_add_plugin({
        name: "rustpg2d",
        version: 1,
        register_plugin: function (importObject) {
            const env = importObject.env;
            env.rustpg2d_now = () => Date.now() / 1000;
            env.rustpg2d_storage_set = (kp, kl, vp, vl) => {
                try {
                    localStorage.setItem(prefix + text(kp, kl), text(vp, vl));
                } catch (e) {
                    console.error("R(ust)PG2D: could not store " + text(kp, kl) + " (" + e + ")");
                }
            };
            env.rustpg2d_storage_remove = (kp, kl) => {
                try {
                    localStorage.removeItem(prefix + text(kp, kl));
                } catch (e) {}
            };
            env.rustpg2d_storage_dump = () => {
                let s = "";
                try {
                    for (let i = 0; i < localStorage.length; i++) {
                        const k = localStorage.key(i);
                        if (k.startsWith(prefix)) {
                            s += k.slice(prefix.length) + "\t" + localStorage.getItem(k) + "\n";
                        }
                    }
                } catch (e) {
                    console.warn("R(ust)PG2D: no browser storage, nothing will be saved (" + e + ")");
                }
                pending = encoder.encode(s);
                return pending.length;
            };
            env.rustpg2d_storage_take = take;
            env.rustpg2d_query_dump = () => {
                pending = encoder.encode(decodeURIComponent(location.search.replace(/^\?/, "").replace(/\+/g, " ")));
                return pending.length;
            };
            env.rustpg2d_query_take = take;
            env.rustpg2d_language_dump = () => {
                pending = encoder.encode(navigator.language || "");
                return pending.length;
            };
        },
    });
})();
