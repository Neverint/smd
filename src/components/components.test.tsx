import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { initI18n } from "../i18n";
import { CheckList } from "./CheckList";
import { ProgressRing } from "./ProgressRing";
import { Segmented } from "./Segmented";
import { Toggle } from "./Toggle";

initI18n();

describe("ProgressRing", () => {
  it("exposes progress to screen readers", () => {
    render(<ProgressRing value={42.4} label="Scanning" />);
    const bar = screen.getByRole("progressbar", { name: "Scanning" });
    expect(bar).toHaveAttribute("aria-valuenow", "42");
  });

  it("has no value when indeterminate", () => {
    render(<ProgressRing label="Working" />);
    expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
  });
});

describe("CheckList", () => {
  const items = [
    { id: "a", label: "Alpha" },
    { id: "b", label: "Beta" },
  ];

  it("select all toggles every item", () => {
    const onChange = vi.fn();
    render(<CheckList label="Items" items={items} selected={new Set()} onChange={onChange} />);
    fireEvent.click(screen.getByLabelText("Select all"));
    expect([...onChange.mock.calls[0][0]]).toEqual(["a", "b"]);
  });

  it("is indeterminate when partly selected", () => {
    render(<CheckList label="Items" items={items} selected={new Set(["a"])} onChange={() => undefined} />);
    expect((screen.getByLabelText("Select all") as HTMLInputElement).indeterminate).toBe(true);
  });
});

describe("Segmented", () => {
  it("moves selection with arrow keys", () => {
    const onChange = vi.fn();
    render(
      <Segmented
        label="Theme"
        value="a"
        onChange={onChange}
        options={[
          { value: "a", label: "A" },
          { value: "b", label: "B" },
        ]}
      />,
    );
    fireEvent.keyDown(screen.getByRole("radio", { name: "A" }), { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith("b");
  });
});

describe("Toggle", () => {
  it("is a switch with state", () => {
    const onChange = vi.fn();
    render(<Toggle checked={false} onChange={onChange} label="Quarantine" />);
    const sw = screen.getByRole("switch", { name: "Quarantine" });
    expect(sw).toHaveAttribute("aria-checked", "false");
    fireEvent.click(sw);
    expect(onChange).toHaveBeenCalledWith(true);
  });
});
