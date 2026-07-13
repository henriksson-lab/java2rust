package idiom.gaps;

public class StaticConstantOwner {
    static class Owner {
        static final int CONSTANT = 7;
    }

    public int combine(int left, int right) {
        return Owner.CONSTANT + Math.max(left, right);
    }
}
