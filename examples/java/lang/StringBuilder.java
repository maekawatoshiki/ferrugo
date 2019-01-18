package java.lang;

public class StringBuilder {
	private final String str;

  public StringBuilder() {
    this.str = "";
  }
	
	public native StringBuilder append(String strAppend);
	public native StringBuilder append(int append);
	// public native StringBuilder append(char append);
	// public native StringBuilder append(boolean append);
	// public native StringBuilder append(float append);
	// public native StringBuilder append(double append);
	// public native StringBuilder append(long append);	
	// public native StringBuilder append(Object append);
  public native String toString();
}
