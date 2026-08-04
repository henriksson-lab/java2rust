public class Fall2 {
    public String run(int v) {
        String s = "";
        switch (v) {
            case 1: s = s + "one";
            case 2: s = s + "two"; break;
            case 3: s = s + "three";
            default: s = s + "other";
        }
        return s;
    }
}
