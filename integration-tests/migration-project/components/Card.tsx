import styles from "../styles/card.module.css";

export function Card({ selected }: { selected: string }) {
  const title = styles["title"];
  const selectedClass = styles[selected];
  return <article className={styles.card}><h2 className={title}>{selectedClass}</h2></article>;
}
