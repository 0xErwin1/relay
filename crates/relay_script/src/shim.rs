//! The JavaScript prelude that builds `pm`, `pm.expect` and `console` over the host functions.

/// The shim installed before every script: it defines `pm` and `console` in terms of the
/// `__relay*` host functions and the `__relayInit` context JSON the sandbox evaluates first.
pub(super) const PRELUDE: &str = r#"
"use strict";
(function () {
  const init = globalThis.__relayInit;
  delete globalThis.__relayInit;

  function stringify(v) {
    if (typeof v === "string") return v;
    if (v === undefined) return "undefined";
    if (v === null) return "null";
    if (typeof v === "number" || typeof v === "boolean") return String(v);
    if (typeof v === "function") return "[Function" + (v.name ? ": " + v.name : "") + "]";
    if (typeof v === "symbol" || typeof v === "bigint") return String(v);
    try { return JSON.stringify(v); } catch (e) { return "[Unserializable]"; }
  }

  function deepEq(a, b) {
    if (a === b) return true;
    if (a === null || b === null || typeof a !== "object" || typeof b !== "object") {
      return typeof a === "number" && typeof b === "number" && Number.isNaN(a) && Number.isNaN(b);
    }
    if (Array.isArray(a) !== Array.isArray(b)) return false;
    const ka = Object.keys(a);
    const kb = Object.keys(b);
    if (ka.length !== kb.length) return false;
    return ka.every((k) => Object.prototype.hasOwnProperty.call(b, k) && deepEq(a[k], b[k]));
  }

  function makeScope(name) {
    return {
      get(key) { return __relayVarGet(name, String(key)); },
      set(key, value) { __relayVarSet(name, String(key), stringify(value)); },
      unset(key) { __relayVarUnset(name, String(key)); },
    };
  }

  function makeExpect(value, customMessage) {
    const prefix = customMessage !== undefined ? String(customMessage) + ": " : "";
    function fail(text) { throw new Error(prefix + text); }
    function assert(ok, text) { if (!ok) fail(text); }
    function show(v) { return stringify(v); }
    function checkType(type) {
      const actual = value === null ? "null" : Array.isArray(value) ? "array" : typeof value;
      assert(actual === type, "expected " + show(value) + " to be a " + type + " (got " + actual + ")");
      return makeAssertion();
    }
    function checkOk() {
      const blank = value === null || value === undefined || value === false || value === ""
        || (typeof value === "number" && (Number.isNaN(value) || value === 0));
      assert(!blank, "expected " + show(value) + " to be ok");
      return makeAssertion();
    }
    function checkFlag(expected) {
      assert(value === expected, "expected " + show(value) + " to be " + String(expected));
      return makeAssertion();
    }
    function checkNull() {
      assert(value === null, "expected " + show(value) + " to be null");
      return makeAssertion();
    }
    function checkEqual(expected) {
      assert(deepEq(value, expected), "expected " + show(value) + " to deeply equal " + show(expected));
      return makeAssertion();
    }
    function checkProperty(name, expected) {
      const has = value !== null && value !== undefined
        && Object.prototype.hasOwnProperty.call(Object(value), name);
      assert(has, "expected " + show(value) + " to have a property " + show(name));
      if (arguments.length > 1) {
        assert(deepEq(value[name], expected), "expected property " + show(name) + " to equal " + show(expected));
      }
      return makeAssertion();
    }
    function checkInclude(expected) {
      if (typeof value === "string") {
        assert(value.includes(String(expected)), "expected " + show(value) + " to include " + show(expected));
      } else if (Array.isArray(value)) {
        assert(value.some((item) => deepEq(item, expected)), "expected " + show(value) + " to include " + show(expected));
      } else if (value !== null && typeof value === "object") {
        const keys = Object.keys(expected);
        const subset = keys.every((k) => Object.prototype.hasOwnProperty.call(value, k) && deepEq(value[k], expected[k]));
        assert(subset, "expected " + show(value) + " to include " + show(expected));
      } else {
        fail("cannot use include on " + show(value));
      }
      return makeAssertion();
    }
    function checkAbove(expected) {
      assert(typeof value === "number" && value > expected, "expected " + show(value) + " to be above " + show(expected));
      return makeAssertion();
    }
    function checkBelow(expected) {
      assert(typeof value === "number" && value < expected, "expected " + show(value) + " to be below " + show(expected));
      return makeAssertion();
    }
    function checkMatch(pattern) {
      assert(new RegExp(pattern).test(String(value)), "expected " + show(value) + " to match " + show(pattern));
      return makeAssertion();
    }
    function okAccessor() {
      checkOk();
      return () => checkOk();
    }
    function flagAccessor(expected) {
      // Asserting on access covers `to.be.true` used as a property; returning a callable that
      // re-asserts covers `to.be.true()` used as a call.
      checkFlag(expected);
      return () => checkFlag(expected);
    }
    function nullAccessor() {
      checkNull();
      return () => checkNull();
    }
    // Chai lets collections chain anywhere (to.be.above, to.have.property), so every link
    // resolves the same table of matchers and never passes an unknown one silently.
    function makeChain() {
      return new Proxy({}, {
        get(_target, prop) {
          switch (String(prop)) {
            case "equal": return checkEqual;
            case "eql": return checkEqual;
            case "include": return checkInclude;
            case "above": return checkAbove;
            case "below": return checkBelow;
            case "match": return checkMatch;
            case "property": return checkProperty;
            case "a":
            case "an": return checkType;
            case "ok": return okAccessor();
            case "true": return flagAccessor(true);
            case "false": return flagAccessor(false);
            case "null": return nullAccessor();
            case "be":
            case "been":
            case "is":
            case "and":
            case "has":
            case "have":
            case "with":
            case "that":
            case "which":
            case "to": return makeChain();
            default: throw new Error("unknown matcher: to." + String(prop));
          }
        },
      });
    }
    function makeAssertion() {
      const assertion = {};
      assertion.to = makeChain();
      return assertion;
    }
    return makeAssertion();
  }

  function makeHeaders(map) {
    return {
      get(name) {
        const lower = String(name).toLowerCase();
        const keys = Object.keys(map);
        for (let i = 0; i < keys.length; i++) {
          if (keys[i].toLowerCase() === lower) return map[keys[i]];
        }
        return undefined;
      },
    };
  }

  const pm = {
    variables: makeScope("variables"),
    environment: makeScope("environment"),
    collectionVariables: makeScope("collectionVariables"),
    globals: makeScope("globals"),
    info: { eventName: init.stage === "test" ? "test" : "prerequest" },
    request: init.request,
    test(name, fn) {
      try {
        if (typeof fn !== "function") throw new Error("pm.test expects a function as its second argument");
        fn();
        __relayTest(String(name), true, undefined);
      } catch (e) {
        __relayTest(String(name), false, e && e.message !== undefined ? String(e.message) : String(e));
      }
    },
    expect: makeExpect,
  };
  if (pm.request) {
    pm.request.headers = makeHeaders(init.request.headers);
    if (typeof pm.request.body === "string") {
      pm.request.body = { raw: pm.request.body };
    } else if (pm.request.body === null) {
      pm.request.body = undefined;
    }
  }
  if (init.response != null) {
    const response = init.response;
    response.headers = makeHeaders(response.headers);
    response.text = function () { return response._text; };
    let cached;
    response.json = function () {
      if (cached === undefined) {
        cached = Object.hasOwn(response, "_parsed") ? response._parsed : JSON.parse(response._text);
      }
      return cached;
    };
    pm.response = response;
  }
  globalThis.pm = pm;
  globalThis.console = {
    log(...args) { __relayConsole("log", args.map(stringify).join(" ")); },
    info(...args) { __relayConsole("info", args.map(stringify).join(" ")); },
    warn(...args) { __relayConsole("warn", args.map(stringify).join(" ")); },
    error(...args) { __relayConsole("error", args.map(stringify).join(" ")); },
  };
})();
"#;
