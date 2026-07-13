class MutualRecursion {
    boolean even(int value) {
        if (value == 0) {
            return true;
        }
        return odd(value - 1);
    }

    boolean odd(int value) {
        if (value == 0) {
            return false;
        }
        return even(value - 1);
    }
}
