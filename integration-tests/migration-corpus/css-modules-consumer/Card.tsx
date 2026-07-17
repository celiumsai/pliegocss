const styles = require("./styles/card.module.css");
import legacy = require("./styles/base.module.css");
const cx = styles;
const { title: heading } = styles;
export const Card = ({ selected }) => <article className={styles.card}><h2 className={cx.title}>{heading}{styles[selected]}</h2><span className={legacy.base} /></article>;
