public class Fallthrough {
    public String run(int v) {
        StringBuilder sb = new StringBuilder();
        switch (v) {
            case 1: sb.append("one");
            case 2: sb.append("two"); break;
            case 3: sb.append("three");
            default: sb.append("other");
        }
        return sb.toString();
    }
}
