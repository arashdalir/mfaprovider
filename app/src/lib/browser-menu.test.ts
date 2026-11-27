import { fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { suppressBrowserMenu } from "./browser-menu";

let stop: () => void;

beforeEach(() => {
  stop = suppressBrowserMenu(document);
});

afterEach(() => {
  stop();
  document.body.innerHTML = "";
});

/** True if a right-click on the element would still open the browser's menu. */
function opensBrowserMenu(html: string, selector = "body > *") {
  document.body.innerHTML = html;
  return fireEvent.contextMenu(document.querySelector(selector)!);
}

describe("suppressBrowserMenu", () => {
  it.each([
    "<p>an account</p>",
    "<button>Copy</button>",
    "<input type='checkbox'>",
    "<input type='radio'>",
    "<input type='file'>",
    "<select><option>All tags</option></select>",
    "<input type='text' disabled>",
  ])("keeps the browser's menu away from %s", (html) => {
    expect(opensBrowserMenu(html)).toBe(false);
  });

  it.each([
    "<input>",
    "<input type='text'>",
    "<input type='password'>",
    "<input type='search'>",
    "<input type='number'>",
    "<textarea></textarea>",
  ])("leaves it in a text box, for pasting: %s", (html) => {
    expect(opensBrowserMenu(html)).toBe(true);
  });

  it("covers what is inside an element too", () => {
    expect(opensBrowserMenu("<button><span>Show</span></button>", "span")).toBe(false);
  });

  it("can be undone", () => {
    stop();
    expect(opensBrowserMenu("<p>text</p>")).toBe(true);
  });
});
