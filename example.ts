import { addToHostState } from "builtin:state";

export default async function demo() {
    const response = await fetch("https://api.ipify.org");
    const text = await response.text();
    console.log("IP:", text);
    console.log("State:", await addToHostState(2));
    console.log("State:", await addToHostState(3));
}
