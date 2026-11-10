import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";
test("opens Brotli invoice, renders pixels and exposes searchable text", async ({
  page,
}) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("data-ready", "true");
  await expect(page.locator("#text")).toContainText("NORDLYS");
  expect(await page.locator("canvas").evaluate((c) => c.width)).toBeGreaterThan(
    100,
  );
  await page.getByRole("searchbox").fill("Nordlys");
  await expect(page.locator("#matches")).toHaveText("1");
  await expect(page.locator(".match")).toHaveCount(1);
  expect(errors).toEqual([]);
});
test("opens lossy JPEG XL example and zooms", async ({ page }) => {
  await page.goto("/#raster-jxl-2");
  await expect(page.locator("html")).toHaveAttribute("data-ready", "true");
  await expect(page.locator("#error")).toBeHidden();
  const width = await page.locator("#canvas").evaluate((c) => c.width);
  await page.getByRole("button", { name: "Zoom in", exact: true }).click();
  await expect
    .poll(() => page.locator("#canvas").evaluate((c) => c.width))
    .toBeGreaterThan(width);
});
test("navigates multipage report with keyboard", async ({ page }) => {
  await page.goto("/#report-brotli");
  await expect(page.locator("html")).toHaveAttribute("data-ready", "true");
  await page.locator("#viewport").focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.locator("#page")).toHaveValue("2");
  await expect(page.locator("#status")).toContainText("Page 2 of");
});
test("rejects legacy profile visibly rather than showing a blank success", async ({
  page,
}) => {
  await page.goto("/#rejected-legacy");
  await expect(page.getByRole("alert")).toContainText("requires PDF 2.0");
  await expect(page.locator("#paper")).toBeHidden();
});
test("local file stays in browser and can be opened", async ({ page }) => {
  const requests = [];
  page.on("request", (r) => {
    if (r.method() !== "GET") requests.push(r.method());
  });
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("data-ready", "true");
  await page.locator("#file").setInputFiles("examples/pdfs/raster-jxl-0.pdf");
  await expect(page.locator("#title")).toHaveText("raster-jxl-0.pdf");
  await expect(page.locator("#status")).toContainText("Page 1 of 1");
  expect(requests).toEqual([]);
});
test("responsive gallery remains usable on a phone viewport", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("data-ready", "true");
  await page.getByRole("button", { name: "Examples", exact: true }).click();
  await expect(page.locator("#sidebar")).toBeVisible();
  await page.locator('[data-example="unicode-brotli"]').click();
  await expect(page.locator("#title")).toHaveText("Words across languages");
  await expect(page.locator("#text")).toContainText("Καλημέρα");
});
test("every gallery example opens or explicitly rejects its profile", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("data-ready", "true");
  const gallery = await page.request
    .get("/web/gallery.json")
    .then((r) => r.json());
  for (const example of gallery) {
    await page.locator(`[data-example="${example.id}"]`).click();
    await expect(page.locator("#title")).toHaveText(example.title);
    if (example.id === "rejected-legacy") {
      await expect(page.getByRole("alert")).toBeVisible();
    } else {
      await expect(page.locator("#status")).toContainText("Page 1 of");
      await expect(page.locator("#error")).toBeHidden();
    }
  }
});
test("late file fetch cannot replace a newer selection", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("data-ready", "true");
  await page.route("**/invoice-flate.pdf", async (route) => {
    await new Promise((r) => setTimeout(r, 200));
    await route.continue();
  });
  await page.locator('[data-example="invoice-flate"]').click();
  await page.locator('[data-example="raster-jxl-2"]').click();
  await expect(page.locator("#title")).toHaveText("Color field · compact");
  await expect(page.locator("#status")).toContainText("Page 1 of");
  await page.waitForTimeout(300);
  await expect(page.locator("#title")).toHaveText("Color field · compact");
});

test("high-DPI zoom stays within the render pixel budget", async ({
  browser,
}) => {
  const context = await browser.newContext({ deviceScaleFactor: 2 });
  const page = await context.newPage();
  await page.goto(test.info().project.use.baseURL);
  await expect(page.locator("html")).toHaveAttribute("data-ready", "true");
  for (let i = 0; i < 12; i++) {
    const button = page.getByRole("button", { name: "Zoom in", exact: true });
    if (await button.isDisabled()) break;
    await button.click();
  }
  await expect(
    page.getByRole("button", { name: "Zoom in", exact: true }),
  ).toBeDisabled();
  await expect(page.locator("#status")).toContainText("Page 1 of");
  await expect(page.locator("#error")).toBeHidden();
  const pixels = await page
    .locator("#canvas")
    .evaluate((c) => c.width * c.height);
  expect(pixels).toBeLessThanOrEqual(16_000_000);
  await context.close();
});
test("embedded release protocol opens bytes and exposes safe links", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("data-ready", "true");
  await page.evaluate(async () => {
    const bytes = await fetch("/tests/skald-corpus/modern-chrome.pdf").then(
      (r) => r.arrayBuffer(),
    );
    const iframe = document.createElement("iframe");
    iframe.id = "embedded-test";
    iframe.style.width = "700px";
    iframe.style.height = "600px";
    window.addEventListener("message", (event) => {
      if (
        event.source === iframe.contentWindow &&
        event.origin === location.origin &&
        event.data?.type === "skald-view:ready"
      )
        iframe.contentWindow.postMessage(
          {
            type: "skald-view:open",
            buffer: bytes,
            title: "Embed test",
            locale: "no",
            source: "/tests/skald-corpus/modern-chrome.pdf",
          },
          location.origin,
          [bytes],
        );
    });
    iframe.src = "/web/embed.html";
    document.body.append(iframe);
  });
  const frame = page.frameLocator("#embedded-test");
  await expect(frame.locator("html")).toHaveAttribute("data-ready", "true");
  await expect(frame.locator("#original")).toHaveText("Åpne original");
  await expect(frame.locator("#links a")).toHaveAttribute(
    "href",
    "https://github.com/beint-no/skald-pdf",
  );
  await expect(frame.locator("#links a")).toHaveAttribute(
    "rel",
    "noopener noreferrer",
  );
});

test("embedded auto-fit ignores height-only changes without resize loops", async ({
  page,
}) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await page.evaluate(async () => {
    const bytes = await fetch(
      "/tests/skald-corpus/reai-16-faktura-signert.pdf",
    ).then((r) => r.arrayBuffer());
    const iframe = document.createElement("iframe");
    iframe.id = "resize-test";
    iframe.style.cssText = "width:300px;height:220px";
    window.embeddedRenderCount = 0;
    window.addEventListener("message", (event) => {
      if (
        event.source === iframe.contentWindow &&
        event.data?.type === "skald-view:rendered"
      )
        window.embeddedRenderCount++;
      if (
        event.source === iframe.contentWindow &&
        event.origin === location.origin &&
        event.data?.type === "skald-view:ready"
      )
        iframe.contentWindow.postMessage(
          { type: "skald-view:open", buffer: bytes, locale: "no" },
          location.origin,
          [bytes],
        );
    });
    iframe.src = "/web/embed.html";
    document.body.append(iframe);
  });
  const frame = page.frameLocator("#resize-test");
  await expect(frame.locator("#status")).toContainText(
    "Signaturen er ikke verifisert",
  );
  await expect
    .poll(() => page.evaluate(() => window.embeddedRenderCount))
    .toBeGreaterThan(0);
  const initialRenders = await page.evaluate(() => window.embeddedRenderCount);
  await page.evaluate(async () => {
    for (const height of [300, 420, 220, 350, 240]) {
      document.querySelector("#resize-test").style.height = `${height}px`;
      await new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      );
    }
  });
  await expect(frame.locator("html")).toHaveAttribute("data-ready", "true");
  expect(await page.evaluate(() => window.embeddedRenderCount)).toBe(
    initialRenders,
  );
  await page.route("**/unsupported-cmyk.pdf", (route) =>
    route.fulfill({
      contentType: "application/pdf",
      body: readFileSync("tests/pdfs/unsupported-cmyk.pdf"),
    }),
  );
  await page.evaluate(async () => {
    const buffer = await fetch("/unsupported-cmyk.pdf").then((r) =>
      r.arrayBuffer(),
    );
    document
      .querySelector("#resize-test")
      .contentWindow.postMessage(
        { type: "skald-view:open", buffer },
        location.origin,
        [buffer],
      );
  });
  await expect(frame.locator("#error")).toBeVisible();
  await page.evaluate(async () => {
    for (let i = 0; i < 5; i++)
      await new Promise((resolve) => requestAnimationFrame(resolve));
  });
  await expect(frame.locator("html")).not.toHaveAttribute("data-ready", "true");
  expect(errors).toEqual([]);
});
