import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import type { NotificationEntry } from "../lib/notificationLog";
import { NotificationCenter } from "./NotificationCenter";

function entry(overrides: Partial<NotificationEntry> = {}): NotificationEntry {
  return {
    id: "wb-1",
    routineId: "r1",
    routineTitle: "Nightly backup",
    message: "Run failed (exit 1)",
    atSecs: Math.floor(Date.now() / 1000) - 60,
    read: false,
    ...overrides,
  };
}

function renderCenter(entries: NotificationEntry[]) {
  const onMarkAllRead = vi.fn();
  const onClear = vi.fn();
  render(
    <MemoryRouter>
      <NotificationCenter entries={entries} onMarkAllRead={onMarkAllRead} onClear={onClear} />
    </MemoryRouter>,
  );
  return { onMarkAllRead, onClear };
}

describe("NotificationCenter", () => {
  it("shows no badge when there are no unread entries", () => {
    renderCenter([entry({ read: true })]);
    expect(screen.queryByText("1")).not.toBeInTheDocument();
  });

  it("shows an unread-count badge on the bell", () => {
    renderCenter([entry(), entry({ id: "wb-2", read: true })]);
    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("caps the displayed badge count at 99+", () => {
    const entries = Array.from({ length: 150 }, (_, i) => entry({ id: `wb-${i}` }));
    renderCenter(entries);
    expect(screen.getByText("99+")).toBeInTheDocument();
  });

  it("panel is closed until the bell is clicked", () => {
    renderCenter([entry()]);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /unread notification/ }));
    expect(screen.getByRole("menu")).toBeInTheDocument();
  });

  it("shows an empty state when there are no notifications", () => {
    renderCenter([]);
    fireEvent.click(screen.getByRole("button", { name: "Notifications" }));
    expect(screen.getByText("No notifications yet")).toBeInTheDocument();
  });

  it("lists entries with routine title and message", () => {
    renderCenter([entry()]);
    fireEvent.click(screen.getByRole("button", { name: /unread notification/ }));
    expect(screen.getByText("Nightly backup")).toBeInTheDocument();
    expect(screen.getByText("Run failed (exit 1)")).toBeInTheDocument();
  });

  it("calls onMarkAllRead when MARK READ is clicked", () => {
    const { onMarkAllRead } = renderCenter([entry()]);
    fireEvent.click(screen.getByRole("button", { name: /unread notification/ }));
    fireEvent.click(screen.getByText("MARK READ"));
    expect(onMarkAllRead).toHaveBeenCalledTimes(1);
  });

  it("does not show MARK READ when nothing is unread", () => {
    renderCenter([entry({ read: true })]);
    fireEvent.click(screen.getByRole("button", { name: "Notifications" }));
    expect(screen.queryByText("MARK READ")).not.toBeInTheDocument();
  });

  it("calls onClear when CLEAR is clicked", () => {
    const { onClear } = renderCenter([entry()]);
    fireEvent.click(screen.getByRole("button", { name: /unread notification/ }));
    fireEvent.click(screen.getByText("CLEAR"));
    expect(onClear).toHaveBeenCalledTimes(1);
  });

  it("closes the panel on Escape", () => {
    renderCenter([entry()]);
    fireEvent.click(screen.getByRole("button", { name: /unread notification/ }));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes the panel on outside click", () => {
    renderCenter([entry()]);
    fireEvent.click(screen.getByRole("button", { name: /unread notification/ }));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
