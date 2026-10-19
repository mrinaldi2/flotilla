// A project's questions for the owner (0.22.0), under More: open or
// answered, for one card or one plan node, a page at a time, each answered
// in place. The tab watches the project so `question_change` pushes arrive.

import { useState } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { QuestionState } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import type { QuestionFilter } from "../../protocol/questions";
import { usePlanWatch } from "../plan/usePlan";
import QuestionCard from "./QuestionCard";
import { useQuestionPages } from "./useQuestionPages";

export interface QuestionsViewProps {
  readonly api: BoardApi;
  readonly projectId: string;
  readonly connected: boolean;
  /** The approve grant: answers and dismisses. */
  readonly canAnswer: boolean;
  /** The plan's nodes, named in the filter; none without a plan. */
  readonly nodes?: readonly PlanNode[];
  readonly onOpenItem?: (id: string) => void;
}

type Shown = "open" | "answered" | "all";

const SHOWN: readonly (readonly [Shown, string])[] = [
  ["open", "Open"],
  ["answered", "Answered"],
  ["all", "All"],
];

const STATE: Readonly<Record<Shown, QuestionState | undefined>> = {
  open: QuestionState.OPEN,
  answered: QuestionState.ANSWERED,
  all: undefined,
};

/** "item:ASL-4", "node:PL-2" or "" for every question. */
type About = string;

function filterOf(projectId: string, shown: Shown, about: About): QuestionFilter {
  const [kind, id] = [about.slice(0, about.indexOf(":")), about.slice(about.indexOf(":") + 1)];
  return {
    projectId,
    state: STATE[shown],
    itemId: kind === "item" ? id : undefined,
    planNodeId: kind === "node" ? id : undefined,
  };
}

/** The cards the questions read so far are on, each once, and the one picked. */
function cardsOf(questions: readonly Question[], about: About): readonly string[] {
  const ids = new Set(questions.flatMap((q) => (q.itemId === undefined ? [] : [q.itemId])));
  if (about.startsWith("item:")) {
    ids.add(about.slice(5));
  }
  // `sort` on a fresh copy, not `toSorted`: the build targets ES2021.
  // oxlint-disable-next-line unicorn/no-array-sort
  return [...ids].sort();
}

function AboutPicker(props: {
  readonly about: About;
  readonly cards: readonly string[];
  readonly nodes: readonly PlanNode[];
  readonly onPick: (about: About) => void;
}): ReactElement {
  return (
    <label className="questions-about">
      <span>About</span>
      <select value={props.about} onChange={(e) => props.onPick(e.target.value)}>
        <option value="">Every card and node</option>
        {props.cards.length === 0 ? null : (
          <optgroup label="Cards">
            {props.cards.map((id) => (
              <option key={id} value={`item:${id}`}>
                {id}
              </option>
            ))}
          </optgroup>
        )}
        {props.nodes.length === 0 ? null : (
          <optgroup label="Plan">
            {props.nodes.map((n) => (
              <option key={n.id} value={`node:${n.id}`}>
                {n.title}
              </option>
            ))}
          </optgroup>
        )}
      </select>
    </label>
  );
}

function emptyText(shown: Shown): string {
  if (shown === "open") {
    return "No question waits for you.";
  }
  return shown === "answered"
    ? "You haven't answered any yet."
    : "No bot has asked you anything yet.";
}

export default function QuestionsView(props: QuestionsViewProps): ReactElement {
  const [shown, setShown] = useState<Shown>("open");
  const [about, setAbout] = useState<About>("");
  usePlanWatch(props.api, props.projectId, props.connected);
  const pages = useQuestionPages(props.api, filterOf(props.projectId, shown, about));
  const open = props.onOpenItem;
  return (
    <div className="questions-view">
      <div className="questions-head">
        <h2>Questions for you</h2>
        <div className="questions-shown" role="group" aria-label="Show">
          {SHOWN.map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={`plan-filter${shown === id ? " plan-filter-on" : ""}`}
              aria-pressed={shown === id}
              onClick={() => setShown(id)}
            >
              {label}
            </button>
          ))}
        </div>
        <AboutPicker
          about={about}
          cards={cardsOf(pages.questions, about)}
          nodes={props.nodes ?? []}
          onPick={setAbout}
        />
      </div>
      {pages.error === null ? null : <p className="chat-note chat-error">{pages.error}</p>}
      {pages.questions.length === 0 && !pages.loading && pages.error === null ? (
        <p className="questions-empty">{emptyText(shown)}</p>
      ) : null}
      <ul className="question-list">
        {pages.questions.map((q) => (
          <QuestionCard
            key={q.id}
            api={props.api}
            question={q}
            canAnswer={props.canAnswer}
            onChanged={pages.apply}
            showWhere
          />
        ))}
      </ul>
      {pages.loading ? <p className="questions-empty">Loading…</p> : null}
      {pages.more === undefined ? null : (
        <button type="button" className="btn btn-small" onClick={pages.more}>
          Show older
        </button>
      )}
      {open === undefined || !about.startsWith("item:") ? null : (
        <button type="button" className="btn btn-small" onClick={() => open(about.slice(5))}>
          {`Open ${about.slice(5)} on the board`}
        </button>
      )}
    </div>
  );
}
