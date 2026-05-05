#!/usr/bin/env python3
"""Generate test PDF fixtures for cpsl-core doc module tests.

Run once to create fixtures, then delete this script (or keep for regeneration).
Requires: pip install fpdf2 reportlab
"""

import os
from fpdf import FPDF
from reportlab.lib.pagesizes import letter
from reportlab.pdfgen import canvas
from reportlab.lib.units import inch
from reportlab.lib import colors

OUT = os.path.dirname(os.path.abspath(__file__))


def simple_text():
    """Single page, simple ASCII text."""
    pdf = FPDF()
    pdf.add_page()
    pdf.set_font("Helvetica", size=12)
    pdf.cell(text="Hello, World!")
    pdf.ln(10)
    pdf.cell(text="This is a simple single-page PDF for testing structural text extraction.")
    pdf.ln(10)
    pdf.cell(text="Line three with some numbers: 12345 and symbols: @#$%")
    pdf.output(os.path.join(OUT, "simple_text.pdf"))


def multi_page():
    """Three pages with distinct content."""
    pdf = FPDF()
    pdf.set_font("Helvetica", size=12)
    for i in range(1, 4):
        pdf.add_page()
        pdf.cell(text=f"Page {i} of 3")
        pdf.ln(10)
        pdf.cell(text=f"Content on page {i}. " * 5)
    pdf.output(os.path.join(OUT, "multi_page.pdf"))


def tables():
    """PDF with a simple table."""
    pdf = FPDF()
    pdf.add_page()
    pdf.set_font("Helvetica", size=10)

    headers = ["Name", "Age", "City"]
    rows = [
        ["Alice", "30", "New York"],
        ["Bob", "25", "London"],
        ["Charlie", "35", "Tokyo"],
        ["Diana", "28", "Paris"],
    ]

    col_width = 50
    row_height = 8

    pdf.set_font("Helvetica", "B", 10)
    for h in headers:
        pdf.cell(col_width, row_height, h, border=1)
    pdf.ln(row_height)

    pdf.set_font("Helvetica", size=10)
    for row in rows:
        for cell in row:
            pdf.cell(col_width, row_height, cell, border=1)
        pdf.ln(row_height)

    pdf.output(os.path.join(OUT, "tables.pdf"))


def unicode_text():
    """PDF with Unicode text — accented Latin, symbols."""
    pdf = FPDF()
    pdf.add_page()
    pdf.set_font("Helvetica", size=12)
    pdf.cell(text="Accented: caf\u00e9, r\u00e9sum\u00e9, na\u00efve, \u00fcber")
    pdf.ln(10)
    pdf.cell(text="Symbols: \u00a9 \u00ae \u00a7 \u00b6 \u00b1 \u00d7 \u00f7")
    pdf.ln(10)
    pdf.cell(text="French: Les \u00e9l\u00e8ves \u00e9tudient \u00e0 l'universit\u00e9")
    pdf.ln(10)
    pdf.cell(text="German: \u00c4rger mit \u00dcbungen und Gr\u00f6\u00dfe")
    pdf.ln(10)
    pdf.cell(text="\u00bfC\u00f3mo est\u00e1 usted? \u00a1Muy bien!")
    pdf.output(os.path.join(OUT, "unicode_text.pdf"))


def form_fields():
    """PDF with interactive form fields using reportlab: text, checkbox, radio."""
    from reportlab.lib.pagesizes import letter
    from reportlab.pdfgen import canvas as rl_canvas
    from reportlab.lib import colors

    path = os.path.join(OUT, "form_fields.pdf")
    c = rl_canvas.Canvas(path, pagesize=letter)
    w, h = letter

    c.setFont("Helvetica", 14)
    c.drawString(72, h - 72, "Test Form")

    # Text field: full_name
    c.setFont("Helvetica", 10)
    c.drawString(72, h - 110, "Full Name:")
    form = c.acroForm
    form.textfield(
        name="full_name",
        x=72, y=h - 135,
        width=200, height=20,
        borderColor=colors.black,
        fillColor=colors.white,
        textColor=colors.black,
        fontSize=10,
        value="",
    )

    # Text field: email
    c.drawString(72, h - 165, "Email:")
    form.textfield(
        name="email",
        x=72, y=h - 190,
        width=200, height=20,
        borderColor=colors.black,
        fillColor=colors.white,
        textColor=colors.black,
        fontSize=10,
        value="",
    )

    # Checkbox: agree_terms
    c.drawString(72, h - 225, "Agree to terms:")
    form.checkbox(
        name="agree_terms",
        x=170, y=h - 230,
        size=15,
        borderColor=colors.black,
        fillColor=colors.white,
        checked=False,
    )

    # Checkbox: subscribe
    c.drawString(72, h - 255, "Subscribe to newsletter:")
    form.checkbox(
        name="subscribe",
        x=210, y=h - 260,
        size=15,
        borderColor=colors.black,
        fillColor=colors.white,
        checked=False,
    )

    # Radio buttons: contact_method
    c.drawString(72, h - 295, "Preferred contact:")
    form.radio(
        name="contact_method",
        value="phone",
        x=72, y=h - 320,
        size=15,
        borderColor=colors.black,
        fillColor=colors.white,
        selected=False,
    )
    c.drawString(92, h - 318, "Phone")

    form.radio(
        name="contact_method",
        value="email_radio",
        x=72, y=h - 345,
        size=15,
        borderColor=colors.black,
        fillColor=colors.white,
        selected=False,
    )
    c.drawString(92, h - 343, "Email")

    c.save()


def utf16_panic():
    """PDF with UTF-16BE encoded text in metadata/streams.

    This encoding pattern has caused panics in pdf-extract.
    """
    content = b"""%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj

2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj

3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]
   /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>
endobj

4 0 obj
<< /Length 44 >>
stream
BT /F1 12 Tf 100 700 Td (Hello UTF-16) Tj ET
endstream
endobj

5 0 obj
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica
   /Encoding /WinAnsiEncoding >>
endobj

6 0 obj
<< /Title (\xfe\xff\x00H\x00e\x00l\x00l\x00o\x00 \x00W\x00o\x00r\x00l\x00d)
   /Author (\xfe\xff\x00T\x00e\x00s\x00t)
>>
endobj

xref
0 7
0000000000 65535 f\r
0000000009 00000 n\r
0000000058 00000 n\r
0000000115 00000 n\r
0000000266 00000 n\r
0000000360 00000 n\r
0000000457 00000 n\r

trailer
<< /Size 7 /Root 1 0 R /Info 6 0 R >>
startxref
610
%%EOF"""

    with open(os.path.join(OUT, "utf16_metadata.pdf"), "wb") as f:
        f.write(content)


def empty_pdf():
    """PDF with a single blank page, no text content."""
    pdf = FPDF()
    pdf.add_page()
    pdf.output(os.path.join(OUT, "empty.pdf"))


def password_protected():
    """PDF encrypted with user password via reportlab."""
    from reportlab.pdfgen import canvas as rl_canvas
    from reportlab.lib.pagesizes import letter as rl_letter
    from reportlab.lib.enums import TA_LEFT

    path = os.path.join(OUT, "password_protected.pdf")
    c = rl_canvas.Canvas(path, pagesize=rl_letter, encrypt="testpass123")
    c.setFont("Helvetica", 12)
    c.drawString(72, 700, "This content is password protected.")
    c.drawString(72, 680, "The password is: testpass123")
    c.save()


if __name__ == "__main__":
    simple_text()
    print("Created: simple_text.pdf")

    multi_page()
    print("Created: multi_page.pdf")

    tables()
    print("Created: tables.pdf")

    unicode_text()
    print("Created: unicode_text.pdf")

    form_fields()
    print("Created: form_fields.pdf")

    utf16_panic()
    print("Created: utf16_metadata.pdf")

    empty_pdf()
    print("Created: empty.pdf")

    password_protected()
    print("Created: password_protected.pdf")

    print(f"\nAll fixtures generated in: {OUT}")
