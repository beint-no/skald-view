import { test, expect } from "@playwright/test";
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
