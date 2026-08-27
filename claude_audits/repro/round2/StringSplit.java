public class StringSplit {
    public static String run() {
        String s = "a,b,,c";
        String[] parts = s.split(",");
        StringBuilder sb = new StringBuilder();
        sb.append(parts.length);
        for (int i = 0; i < parts.length; i++) { sb.append("[").append(parts[i]).append("]"); }
        String[] ws = "one two  three".split("\\s+");
        sb.append("|").append(ws.length);
        return sb.toString();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
