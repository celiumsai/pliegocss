import styles from "./styles/card.module.css";
export const Card = ({ selected }) => <article className={styles.card}><h2 className={styles.title}>{styles[selected]}</h2></article>;
