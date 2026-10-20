// Card ids in Markdown a bot or the owner wrote (UX-035 §1, §7): ids in text
// become card links; code blocks and links are left alone, and inline code
// is linked only when the whole span is an id. Every other link stays inert
// text (`SAFE_LINKS`): bot output is untrusted.

import { useMemo } from "react";
import type { ReactElement, ReactNode } from "react";
import { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import { isCardId, splitCardIds } from "./cardIds";
import CardLink from "./CardLink";
import { useCardLinks } from "./CardLinks";
import { linkedId, SCHEME } from "./cardScheme";

interface MdNode {
  type: string;
  value?: string;
  url?: string;
  children?: MdNode[];
}

const SKIP = new Set(["link", "linkReference", "code", "html"]);

function cardLink(id: string, inner: MdNode): MdNode {
  return { type: "link", url: `${SCHEME}${id}`, children: [inner] };
}

function linkIds(node: MdNode, pattern: RegExp): void {
  if (node.children === undefined || SKIP.has(node.type)) {
    return;
  }
  node.children = node.children.flatMap((child): MdNode[] => {
    if (child.type === "text" && child.value !== undefined) {
      return splitCardIds(child.value, pattern).map((s) =>
        s.kind === "text"
          ? { type: "text", value: s.text }
          : cardLink(s.id, { type: "text", value: s.id }),
      );
    }
    if (
      child.type === "inlineCode" &&
      child.value !== undefined &&
      isCardId(child.value, pattern)
    ) {
      return [cardLink(child.value, child)];
    }
    linkIds(child, pattern);
    return [child];
  });
}

/** The remark plugin for `pattern`. */
function remarkCardIds(pattern: RegExp | null) {
  return () => (tree: MdNode) => {
    if (pattern !== null) {
      linkIds(tree, pattern);
    }
  };
}

function urlTransform(url: string): string {
  return linkedId(url) !== null ? url : defaultUrlTransform(url);
}

type Anchor = (props: { children?: ReactNode; href?: string }) => ReactElement;

/**
 * The props a `ReactMarkdown` takes to link card ids. `inert` draws every
 * other link; a written `flotilla://item/` link (or one from before the
 * rename, `hermes://item/`) is a card link only when its
 * id is one, so a bot can't put other words behind it.
 */
export function useCardMarkdown(inert: Anchor) {
  const pattern = useCardLinks()?.pattern ?? null;
  return useMemo(() => {
    const a: Anchor = ({ children, href }) => {
      const id = linkedId(href);
      return id !== null && pattern !== null && isCardId(id, pattern) ? (
        <CardLink id={id} />
      ) : (
        inert({ children, href })
      );
    };
    // GitHub's Markdown (0.23.0): tables, task lists and strikethrough in
    // PRDs, designs and research. The links it finds in text stay inert.
    return { remarkPlugins: [remarkGfm, remarkCardIds(pattern)], urlTransform, a };
  }, [pattern, inert]);
}
