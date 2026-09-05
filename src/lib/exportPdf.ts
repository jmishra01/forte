import jsPDF from "jspdf";
import html2canvas from "html2canvas";
import { exportApi } from "./api";

export async function exportNoteToPdf(title: string, element: HTMLElement): Promise<boolean> {
  const canvas = await html2canvas(element, { backgroundColor: "#ffffff", scale: 2 });
  const imgData = canvas.toDataURL("image/png");

  const pdf = new jsPDF({ unit: "pt", format: "a4" });
  const pageWidth = pdf.internal.pageSize.getWidth();
  const pageHeight = pdf.internal.pageSize.getHeight();
  const imgWidth = pageWidth;
  const imgHeight = (canvas.height * imgWidth) / canvas.width;

  let remainingHeight = imgHeight;
  let position = 0;
  pdf.addImage(imgData, "PNG", 0, position, imgWidth, imgHeight);
  remainingHeight -= pageHeight;

  while (remainingHeight > 0) {
    position -= pageHeight;
    pdf.addPage();
    pdf.addImage(imgData, "PNG", 0, position, imgWidth, imgHeight);
    remainingHeight -= pageHeight;
  }

  const path = await exportApi.pickPdfSavePath(`${title || "note"}.pdf`);
  if (!path) return false;
  const bytes = new Uint8Array(pdf.output("arraybuffer") as ArrayBuffer);
  await exportApi.writeFileBytes(path, bytes);
  return true;
}
