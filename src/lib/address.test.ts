// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
import assert from "node:assert/strict";
import { test } from "node:test";
import { parseAddress, recentHosts } from "./address.ts";

test("a plain IPv4 address needs no port", () => {
  assert.deepEqual(parseAddress("192.168.1.20"), { host: "192.168.1.20" });
  assert.deepEqual(parseAddress("  10.0.0.5 "), { host: "10.0.0.5" });
});

test("an explicit port is still accepted", () => {
  assert.deepEqual(parseAddress("192.168.1.20:41220"), { host: "192.168.1.20", port: 41220 });
  assert.equal(parseAddress("192.168.1.20:0"), null);
  assert.equal(parseAddress("192.168.1.20:65536"), null);
  assert.equal(parseAddress("192.168.1.20:"), null);
});

test("anything that is not IPv4 is refused", () => {
  for (const text of ["", "192.168.1", "192.168.1.256", "192.168.1.20.5", "host.local", "fe80::1", "[::1]:41220", "1.2.3.4 5", "a.b.c.d", "1.2.3.4:x"]) {
    assert.equal(parseAddress(text), null, text);
  }
});

test("leading zeros are normalized so the same computer is not listed twice", () => {
  assert.deepEqual(parseAddress("192.168.001.020"), { host: "192.168.1.20" });
});

test("recent targets show each host once, newest first", () => {
  assert.deepEqual(recentHosts(["192.168.1.20:41220", "10.0.0.2:5000", "192.168.1.20:5123", "junk"]), ["192.168.1.20", "10.0.0.2"]);
});
