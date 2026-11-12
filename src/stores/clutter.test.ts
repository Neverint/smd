import { describe, expect, it } from "vitest";
import { allButOne } from "./clutter";

describe("duplicate selection", () => {
  it("selects every copy except the first of each group", () => {
    const groups = [{ files: [{ path: "a" }, { path: "b" }, { path: "c" }] }, { files: [{ path: "x" }, { path: "y" }] }];
    expect(allButOne(groups)).toEqual(["b", "c", "y"]);
    expect(allButOne([])).toEqual([]);
  });
});
