package examples.control;

public class SwitchFallthrough {
    public int score(String grade) {
        int value = 0;
        switch (grade) {
            case "A":
                value += 4;
                break;
            case "B":
            case "C":
                value += 2;
                break;
            default:
                value -= 1;
        }
        return value;
    }
}

