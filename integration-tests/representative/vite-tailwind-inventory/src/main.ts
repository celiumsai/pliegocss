import "./app.css";

const channel = "stable";
const channelClass = channel === "stable" ? "text-emerald-400" : "text-amber-400";

document.documentElement.dataset.channel = channelClass;
