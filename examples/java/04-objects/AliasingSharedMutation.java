package examples.objects;

public class AliasingSharedMutation {
    static class Box {
        int value;
    }

    public int observe() {
        Box first = new Box();
        Box second = first;
        first.value = 7;
        second.value += 1;
        return first.value;
    }
}

