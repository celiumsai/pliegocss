import styles from "./styles/Card.module.css";

const cardStyles = styles;
const { title: heading } = styles;

export function Card({ selected }: { selected: boolean }) {
  const state = selected ? styles.selected : styles.card;
  return (
    <article className={`${styles.card} ${state}`}>
      <h2 className={cardStyles.title}>{heading}</h2>
    </article>
  );
}
