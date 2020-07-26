#! python3
from tkinter.filedialog import askopenfilename

filename = askopenfilename()
if not filename:
    print("")
else:
    print(filename)
