import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "@/App";
import { Toaster } from "@/ui/Toaster";

type Call = { url: string; method: string; body?: unknown };
const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
let calls: Call[] = [];
let emailOn = true;
let answer: Record<string, unknown> = {};
const me = () => ({
  id: 1,
  email: "mai@example.com",
  name: "Mai Tran",
  is_super_admin: true,
  csrf_token: "c".repeat(64),
  email_enabled: emailOn,
  companies: [{ slug: "acme", name: "Acme Studio", role: "owner" }],
});
const server = () => ({
  version: "0.1.0",
  uptime_secs: 10,
  data_bytes: 1,
  companies: 1,
  public_url: "http://localhost:8791",
  email_server: emailOn
    ? {
        host: "smtp.example.com",
        port: 587,
        from: "Tazko <no-reply@example.com>",
        signs_in: true,
      }
    : null,
  email_queue: {
    pending: 1,
    sent_24h: 14,
    failed_24h: 0,
    last_error: { error: "recipient: 550 no such user", at: 1_700_000_000 },
  },
  last_backup: null,
  last_restore_test: null,
});

beforeEach(() => {
  calls = [];
  emailOn = true;
  answer = {
    result: "invited",
    emailed: true,
    email_limited: false,
    share_url: "http://localhost:8791",
  };
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init: RequestInit = {}) => {
      const method = init.method ?? "GET";
      calls.push({
        url,
        method,
        body: typeof init.body === "string" ? JSON.parse(init.body) : undefined,
      });
      if (url === "/api/v1/me") return json(me());
      if (url === "/api/v1/me/settings") return json({});
      if (url === "/api/v1/c/acme/settings")
        return json({
          name: "Acme Studio",
          slug: "acme",
          members: [],
          invites: [],
          storage_used: 0,
          storage_limit: null,
        });
      if (url === "/api/v1/c/acme/invites") return json(answer, 201);
      if (url === "/api/v1/admin/server") return json(server());
      if (url === "/api/v1/admin/email/test")
        return json({ to: "mai@example.com" }, 202);
      return json({ error: "not found" }, 404);
    }),
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  window.history.pushState(null, "", "/");
});

function open(path: string) {
  window.history.pushState(null, "", path);
  render(
    <Toaster>
      <App />
    </Toaster>,
  );
}

async function invite(email: string) {
  fireEvent.change(
    await screen.findByRole("textbox", { name: "Email address" }),
    { target: { value: email } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Invite" }));
}

describe("emailed invitations", () => {
  it("sends the invitation by email when ticked and says so", async () => {
    open("/c/acme/settings");
    const box = (await screen.findByRole("checkbox", {
      name: "Email the invitation",
    })) as HTMLInputElement;
    expect(box.checked).toBe(true);
    await invite("new@example.com");
    expect(
      await screen.findByText("Invitation emailed to new@example.com"),
    ).toBeTruthy();
    expect(calls.find((c) => c.method === "POST")?.body).toEqual({
      email: "new@example.com",
      role: "member",
      send_email: true,
    });
  });

  it("shows the address to share when email is off or limited", async () => {
    emailOn = false;
    answer = {
      result: "invited",
      emailed: false,
      email_limited: false,
      share_url: "http://localhost:8791",
    };
    open("/c/acme/settings");
    await screen.findByRole("textbox", { name: "Email address" });
    expect(
      screen.queryByRole("checkbox", { name: "Email the invitation" }),
    ).toBeNull();
    await invite("new@example.com");
    expect(
      await screen.findByText(/Share http:\/\/localhost:8791 with them/),
    ).toBeTruthy();
  });

  it("shows the email status on the admin Server page and sends a test", async () => {
    open("/admin/server");
    expect(await screen.findByText("Configured")).toBeTruthy();
    expect(screen.getByText(/smtp.example.com:587/)).toBeTruthy();
    expect(screen.getByText(/14 sent, 0 failed/)).toBeTruthy();
    expect(screen.getByText(/550 no such user/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Send test email" }));
    await waitFor(() =>
      expect(
        calls.some(
          (c) => c.method === "POST" && c.url === "/api/v1/admin/email/test",
        ),
      ).toBe(true),
    );
    expect(
      await screen.findByText("Test email on its way to mai@example.com"),
    ).toBeTruthy();
  });
});
