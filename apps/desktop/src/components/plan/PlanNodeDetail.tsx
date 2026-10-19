// A plan node (0.21.0, canvas D1): where it sits, its stages with the gates
// marked, its research and the spikes that close it, its documents (each
// read in place), its cards and its history. A gate waiting for the owner
// opens from its header.

import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { Bot } from "../../protocol/entities";
import type { ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type {
  PlanDoc,
  PlanNode,
  PlanNodeDetail as PlanNodeDetailMessage,
} from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { PlanDocKind, PlanResearchState } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import Markdown from "../control/Markdown";
import { typeLabel } from "../board/labels";
import { SkipResearch } from "./PlanGatePanel";
import PlanNodeActions from "./PlanNodeActions";
import PlanNodeQuestions from "./PlanNodeQuestions";
import { day, historyLine } from "./planHistory";
import PlanStepper from "./PlanStepper";
import { docWord, fileName, kindWords, stageChip } from "./planText";
import { usePlanNode } from "./usePlanNode";

export interface PlanNodeDetailProps {
  readonly api: BoardApi;
  readonly node: PlanNode;
  /** Its ancestors, the goal first. */
  readonly ancestors: readonly PlanNode[];
  readonly who: (actor: string) => string;
  /** The owner's: rules gates, skips research, moves a goal, drops a node. */
  readonly canRule: boolean;
  /** The control grant: edits a node. */
  readonly canEdit: boolean;
  readonly bots: readonly Bot[];
  readonly leadName?: string;
  readonly onOpenNode: (id: string) => void;
  readonly onOpenGate: () => void;
  readonly onOpenDoc: (doc: PlanDoc) => void;
  readonly onOpenItem?: (id: string) => void;
  readonly onOpenBoard?: (boardId: string) => void;
  readonly onChanged: (node: PlanNode) => void;
}

const RESEARCH_STATE: Readonly<Record<PlanResearchState, string>> = {
  [PlanResearchState.UNSPECIFIED]: "Open",
  [PlanResearchState.OPEN]: "Open",
  [PlanResearchState.CLOSED]: "Closed",
  [PlanResearchState.SKIPPED]: "Skipped",
};

function CardRow(props: {
  readonly card: ItemCard | undefined;
  readonly id: string;
  readonly doc?: PlanDoc;
  readonly onOpenItem?: (id: string) => void;
  readonly onOpenDoc: (doc: PlanDoc) => void;
}): ReactElement {
  const { card, id, doc } = props;
  const open = props.onOpenItem;
  return (
    <li className="plan-card-row">
      <span className="plan-card-line">
        <span className="mono plan-dim">{id}</span>
        {open === undefined ? (
          <span className="plan-card-title">{card?.title ?? ""}</span>
        ) : (
          <button type="button" className="plan-link plan-card-title" onClick={() => open(id)}>
            {card?.title ?? id}
          </button>
        )}
        <span className="plan-dim">
          {card === undefined ? "" : `${typeLabel(card.type).word} · ${card.columnKey}`}
        </span>
      </span>
      {doc === undefined ? null : (
        <button type="button" className="plan-link mono" onClick={() => props.onOpenDoc(doc)}>
          {fileName(doc.path)}
        </button>
      )}
    </li>
  );
}

type ResearchProps = Pick<
  PlanNodeDetailProps,
  "api" | "node" | "canRule" | "onOpenItem" | "onOpenDoc" | "onChanged"
> & { readonly cards: readonly ItemCard[]; readonly questions: ReactElement | null };

function Research(props: ResearchProps): ReactElement | null {
  const { node } = props;
  const research = node.research;
  if (research === undefined) {
    return null;
  }
  const state = RESEARCH_STATE[research.state];
  const when = research.closedAt === undefined ? "" : ` ${day(research.closedAt)}`;
  return (
    <section className="plan-section" aria-label="Research">
      <div className="plan-section-head">
        <h3>Research</h3>
        <span
          className={`plan-chip plan-chip-${research.state === PlanResearchState.OPEN ? "research" : "plain"}`}
        >
          {`${state}${when}`}
        </span>
      </div>
      {research.skippedReason ? (
        <p className="plan-line">{`Skipped: ${research.skippedReason}`}</p>
      ) : null}
      {research.spikes.length === 0 ? (
        <p className="plan-empty">
          No spikes yet. The lead opens them on the initiative&apos;s board.
        </p>
      ) : (
        <ul className="plan-cards">
          {research.spikes.map((id) => (
            <CardRow
              key={id}
              id={id}
              card={props.cards.find((c) => c.id === id)}
              doc={node.docs.find((d) => d.kind === PlanDocKind.RESEARCH && d.itemId === id)}
              onOpenItem={props.onOpenItem}
              onOpenDoc={props.onOpenDoc}
            />
          ))}
        </ul>
      )}
      {props.questions}
      {props.canRule && research.state === PlanResearchState.OPEN && node.gate === undefined ? (
        <SkipResearch api={props.api} node={node} onSkipped={props.onChanged} />
      ) : null}
    </section>
  );
}

/** "Approved 15 Oct @ 9b2f0c1", "Changed since approval", "Not approved yet". */
export function docState(doc: PlanDoc): string {
  if (doc.changedSinceApproval) {
    return "Changed since approval";
  }
  if (doc.approvedSha !== undefined) {
    const on = day(doc.approvedAt);
    return `Approved${on === "" ? "" : ` ${on}`} @ ${doc.approvedSha}`;
  }
  return doc.kind === PlanDocKind.RESEARCH ? "Research" : "Not approved yet";
}

function Documents(props: Pick<PlanNodeDetailProps, "node" | "onOpenDoc">): ReactElement | null {
  const docs = props.node.docs.filter((d) => d.kind !== PlanDocKind.RESEARCH);
  if (docs.length === 0) {
    return null;
  }
  return (
    <section className="plan-section" aria-label="Documents">
      <h3>Documents</h3>
      <ul className="plan-docs">
        {docs.map((doc) => (
          <li key={doc.path} className={doc.changedSinceApproval ? "plan-doc-changed" : ""}>
            <span className="plan-doc-kind">{docWord(doc.kind)}</span>
            <button type="button" className="plan-link mono" onClick={() => props.onOpenDoc(doc)}>
              {fileName(doc.path)}
            </button>
            <span className="plan-dim">
              {doc.changedSinceApproval ? <span aria-hidden="true">⚠ </span> : null}
              {docState(doc)}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Header(
  props: Pick<PlanNodeDetailProps, "node" | "who" | "canRule" | "onOpenGate" | "onOpenBoard">,
): ReactElement {
  const { node } = props;
  const kind = kindWords(node.kind);
  const chip = stageChip(node);
  const lead = node.lead === undefined ? null : props.who(`bot:${node.lead}`);
  const board = node.boardId;
  const open = props.onOpenBoard;
  return (
    <div className="plan-node-head">
      <span className={`plan-glyph plan-glyph-${kind.word.toLowerCase()}`} aria-hidden="true">
        {kind.glyph}
      </span>
      <div className="plan-node-titles">
        <h1>{node.title}</h1>
        <span className="plan-dim">
          {kind.word} · <span className={`plan-chip plan-chip-${chip.tone}`}>{chip.text}</span>
          {lead === null ? "" : ` · lead ${lead}`}
          {board !== undefined && open !== undefined ? (
            <>
              {" · "}
              <button type="button" className="plan-link" onClick={() => open(board)}>
                Open its board
              </button>
            </>
          ) : null}
        </span>
      </div>
      {node.gate === undefined ? null : (
        <button type="button" className="btn btn-primary" onClick={props.onOpenGate}>
          {props.canRule ? "Review the gate" : "See the gate"}
        </button>
      )}
    </div>
  );
}

/** Research with the node's questions under it, or the questions on their own, then Documents. */
function Sections(
  props: PlanNodeDetailProps & { readonly detail: PlanNodeDetailMessage | null },
): ReactElement {
  const { node, detail } = props;
  const questions =
    detail === null || detail.questions.length === 0 ? null : (
      <PlanNodeQuestions
        api={props.api}
        nodeId={node.id}
        cards={detail.cards}
        questions={detail.questions}
        canAnswer={props.canRule}
      />
    );
  return (
    <div className="plan-sections">
      <Research {...props} cards={detail?.cards ?? []} questions={questions} />
      {node.research === undefined && questions !== null ? (
        <section className="plan-section" aria-label="Questions for you">
          {questions}
        </section>
      ) : null}
      <Documents {...props} />
    </div>
  );
}

export default function PlanNodeDetail(props: PlanNodeDetailProps): ReactElement {
  const { node } = props;
  const state = usePlanNode(props.api, node.id, node.version);
  const detail = state.status === "ready" ? state.detail : null;
  const cards = detail?.cards ?? [];
  const spikes = new Set(node.research?.spikes ?? []);
  const own = cards.filter((c) => !spikes.has(c.id));
  return (
    <main className="plan-main" aria-label={node.title}>
      {props.ancestors.length === 0 ? null : (
        <nav className="plan-crumbs" aria-label="Breadcrumb">
          {props.ancestors.map((a) => (
            <span key={a.id}>
              <button type="button" className="plan-link" onClick={() => props.onOpenNode(a.id)}>
                {a.title}
              </button>
              {" › "}
            </span>
          ))}
        </nav>
      )}
      <Header {...props} />
      <PlanNodeActions
        key={node.id}
        api={props.api}
        node={node}
        bots={props.bots}
        leadName={props.leadName}
        canRule={props.canRule}
        canEdit={props.canEdit}
        onChanged={props.onChanged}
      />
      <PlanStepper node={node} />
      {node.summary.trim() === "" ? null : (
        <div className="plan-summary">
          <Markdown>{node.summary}</Markdown>
        </div>
      )}
      <Sections {...props} detail={detail} />
      {own.length === 0 ? null : (
        <section className="plan-section" aria-label="Cards">
          <h3>
            {node.cardsTotal === 0 ? "Cards" : `Cards · ${node.cardsDone}/${node.cardsTotal} done`}
          </h3>
          <ul className="plan-cards">
            {own.map((c) => (
              <CardRow
                key={c.id}
                id={c.id}
                card={c}
                onOpenItem={props.onOpenItem}
                onOpenDoc={props.onOpenDoc}
              />
            ))}
          </ul>
        </section>
      )}
      <section className="plan-history" aria-label="History">
        <h3>History</h3>
        {state.status === "failed" ? <p className="plan-empty">{state.message}</p> : null}
        {state.status === "loading" ? <p className="plan-empty">Loading…</p> : null}
        {detail === null ? null : (
          <ol>
            {detail.history.map((e) => (
              <li key={`${e.at?.seconds ?? 0n}-${e.kind}-${e.actor}`}>
                <span className="plan-when">{day(e.at)}</span>
                <span>{historyLine(e, props.who)}</span>
              </li>
            ))}
          </ol>
        )}
      </section>
    </main>
  );
}
