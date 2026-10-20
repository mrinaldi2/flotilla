import type { Story } from "@ladle/react";
import { useRef, useState } from "react";
import type { ReactElement } from "react";
import { card } from "../../test/boardFixtures";
import { node } from "../../test/planFixtures";
import {
  calendarQuestion,
  fontQuestion,
  licensingQuestion,
  questionsClient,
  saintsQuestion,
} from "../../test/questionFixtures";
import { useClickOnce } from "../../test/storyClick";
import BoardCard from "../board/BoardCard";
import QuestionsSection from "./QuestionsSection";
import QuestionsView from "./QuestionsView";

const noop = (): void => {};

function Section(props: { readonly canAnswer: boolean }): ReactElement {
  const [client] = useState(() => questionsClient());
  const [questions, setQuestions] = useState(() => [
    saintsQuestion(),
    calendarQuestion(),
    licensingQuestion(),
    fontQuestion(),
  ]);
  return (
    <div className="item-drawer" style={{ position: "static", height: "auto", padding: 16 }}>
      <QuestionsSection
        api={client}
        questions={questions}
        canAnswer={props.canAnswer}
        onChanged={(q) => setQuestions((all) => all.map((x) => (x.id === q.id ? q : x)))}
      />
    </div>
  );
}

/** A card's questions: open ones with their options and reply box, then answered and dismissed. */
export const OnACard: Story = () => <Section canAnswer />;
/** The same to a connection without the approve grant: the options, read only. */
export const ReadOnly: Story = () => <Section canAnswer={false} />;

function Project(props: { readonly click?: string }): ReactElement {
  const box = useRef<HTMLDivElement>(null);
  const [client] = useState(() =>
    questionsClient([
      saintsQuestion(),
      calendarQuestion(),
      licensingQuestion(),
      fontQuestion(),
    ]).onBoard("boardWatch", () => {
      throw new Error("no board in this story");
    }),
  );
  useClickOnce(box, props.click);
  return (
    <div className="main" ref={box} style={{ height: 760 }}>
      <QuestionsView
        api={client}
        projectId="p1"
        connected
        canAnswer
        nodes={[node({ id: "PL-2", title: "Saints dataset" })]}
        onOpenItem={noop}
      />
    </div>
  );
}

/** The project's Questions tab, open ones first. */
export const ProjectOpen: Story = () => <Project />;
/** Every question, answered and dismissed too. */
export const ProjectAll: Story = () => <Project click="All" />;

/** A card on the board with questions waiting for the owner. */
export const BoardCardBadge: Story = () => (
  <div className="board-view" style={{ width: 280, padding: 16 }}>
    <BoardCard
      card={card({ id: "ASL-4", title: "Sources and licensing", questionsOpen: 2 })}
      columnName="Research"
      assignee={undefined}
      onOpenMoveMenu={null}
      onDragStart={noop}
      onDragEnd={noop}
      dragging={false}
    />
  </div>
);
