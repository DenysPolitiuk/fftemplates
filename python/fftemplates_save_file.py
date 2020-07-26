#! python3
from tkinter.filedialog import asksaveasfilename

filename = asksaveasfilename()
if not filename:
    print("")
else:
    print(filename)
