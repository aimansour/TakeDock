const en = {
  duration: 'Duration',
  durationUnavailable: 'Duration unavailable',
  hours: 'hours',
  minutes: 'minutes',
  seconds: 'seconds',
  milliseconds: 'milliseconds',
  destination_setup_failed:
    'The default destination could not be created. Choose a folder in Settings.',
  settings_save_failed: 'Settings could not be saved.',
  activity: 'Activity',
  activityProgress: 'Activity progress',
  copying: 'Copying video',
  verifying: 'Verifying copied video',
  deleting: 'Deleting video',
  renaming: 'Renaming video',
  preparing: 'Preparing operation',
  loadingVideos: 'Loading videos',
  windowName: 'Window name',
  renameWindow: 'Save window name',
  newWindow: 'New window',
  window_renamed: 'Window name saved',
  windowHelp:
    'Ctrl+Shift+N opens another window. Phone, destination and settings are shared.',
  invalid_window_name:
    'Enter a window name of 1 to 80 characters without control characters.',
  updates: 'Updates',
  updateHelp:
    'Checks run automatically at startup. Installation starts only when you choose Install update. Installation closes TakeDock and cancels pending operations.',
  checkUpdates: 'Check for updates',
  installUpdate: 'Install update',
  update_checking: 'Checking for updates…',
  update_available: 'An update is available.',
  update_current: 'You have the latest release.',
  update_installing: 'Downloading and verifying the update…',
  update_installed: 'Update installed.',
  update_error: 'The update operation failed. You can try checking again.',
  recording: 'Recording',
  videos: 'Videos',
  settings: 'Settings',
  start: 'Start video',
  stop: 'Stop',
  deleteTake: 'Delete this take',
  stopAndDelete: 'Stop and delete',
  discardShortcutHelp:
    'Deletion requires the Stop and delete button; Ctrl+Shift+R only starts or stops recording.',
  discardAssumptionHelp:
    'Automatic deletion requires Open Camera to save here with no other app adding videos during the take.',
  discard_waiting_to_stop: 'Waiting for the take to be ready to stop…',
  discard_confirming_stop: 'Confirming that recording has stopped…',
  discard_finalizing: 'Waiting for the video to finish saving…',
  discard_deleting: 'Deleting this take…',
  discard_completed: 'The take was deleted.',
  discard_retained: 'Automatic deletion was not completed.',
  discard_outcome_unknown:
    'Deletion was authorized, but its result could not be confirmed. Check Videos before taking further action.',
  discard_no_managed_take:
    'Automatic deletion is available only for a take started through TakeDock.',
  discard_ready_for_start:
    'Start a take through TakeDock to make per-take deletion available.',
  discard_not_confirmed_idle:
    'The take did not start from a confirmed idle camera. Stop normally, then select the video in Videos.',
  discard_baseline_unavailable:
    'The file list was not ready before this take started. Stop normally, then select the video in Videos.',
  discard_file_history_lost:
    'The take could not be associated with one unchanged video. Check Videos and select the file explicitly.',
  discard_take_replaced:
    'This request belongs to a previous take. Check the current recording state.',
  discard_capture_changed:
    'Another recording action cancelled the pending deletion.',
  discard_observation_lost:
    'The camera state could not be confirmed. No pending deletion will continue.',
  discard_start_unconfirmed:
    'Recording start was not confirmed. The take will not be deleted automatically.',
  discard_stop_unconfirmed:
    'Recording stop was not confirmed. Check the phone; automatic deletion has been cancelled.',
  discard_source_changed:
    'The video changed while deletion was being prepared. Check Videos.',
  discard_inspection_failed:
    'The video could not be checked for safe deletion. Check Videos.',
  discard_finalization_timeout:
    'The video did not finish saving in time. Check Videos before deleting it.',
  discard_cancelled: 'Pending deletion was cancelled.',
  discard_disconnected:
    'The connection changed. No pending deletion will continue.',
  discard_worker_unavailable: 'Deletion could not be queued. Check Videos.',
  discard_capture_pending:
    'A recording action is still being confirmed. Check the recording state before trying again.',
  discard_busy: 'A previous deletion is still finishing.',
  discard_delete_failed: 'The take could not be deleted. Check Videos.',
  discard_permit_timeout:
    'Deletion was not authorized in time. The video was not deleted.',
  discard_permit_failed:
    'Deletion authorization failed. The video was not deleted.',
  discard_invalid_operation:
    'The deletion request is invalid. The video was not deleted.',
  pause: 'Pause video',
  resume: 'Resume video',
  idle: 'Ready',
  unknown: 'Unavailable',
  paused: 'Paused',
  recordingState: 'Recording',
  connection: 'Phone connection',
  connected: 'Connected',
  disconnected: 'No authorized phone connected',
  ready: 'Ready',
  open_camera_foreground:
    'Open Open Camera on the phone and keep it in the foreground.',
  select_video_mode: 'Select video mode in Open Camera.',
  observer_starting: 'Starting camera observer…',
  observer_setup_failed:
    'Camera observer unavailable. Video files are still accessible.',
  multiple_devices: 'Connect exactly one authorized phone.',
  verification_unconfirmed:
    'The last recording command could not be confirmed. Check the phone, then reconnect.',
  unsupported_camera_state:
    'Camera state is unavailable. Check Open Camera and its volume-key settings.',
  observer_closed: 'Camera observer stopped. Reconnect to resume controls.',
  command_delivery_failed: 'The recording command could not be delivered.',
  captureHelp:
    'Open Camera must be active, in video mode, with volume up configured for start/stop and volume down for pause/resume.',
  recordingKeys: 'Ctrl+Shift+R: start/stop · Ctrl+Shift+P: pause/resume',
  refresh: 'Refresh videos',
  selectAll: 'Select all completed videos',
  clear: 'Clear selection',
  copy: 'Copy to computer',
  move: 'Move to computer',
  delete: 'Delete from phone',
  rename: 'Rename video',
  name: 'Name',
  size: 'Size',
  modified: 'Modified',
  state: 'State',
  select: 'Select',
  complete: 'Completed',
  protected: 'Protected: unfinished or unrecognized video',
  empty: 'No videos found. Refresh while your phone is connected.',
  selected: 'Selected',
  newName: 'New name (without extension)',
  saveRename: 'Save name',
  cancel: 'Cancel',
  confirmDelete: 'Confirm deletion',
  deleteHelp: 'Delete the selected videos permanently from the phone?',
  count: 'videos',
  jobs: 'File operations',
  completed: 'Completed',
  failed: 'Failed',
  cancelled: 'Cancelled',
  skipped: 'Skipped',
  queued: 'Queued',
  running: 'In progress',
  result: 'Result',
  language: 'Language',
  destination: 'Computer destination folder',
  chooseFolder: 'Choose folder',
  adbPath: 'ADB executable (optional)',
  chooseAdb: 'Choose ADB executable',
  successSound: 'Play a sound on success',
  readySound: 'Play the ready-to-speak cue',
  readyDelay: 'Speech-readiness delay after confirmation (milliseconds)',
  readyDelayHelp:
    'The delay starts after confirmation and changes apply to the next Start. Reduce it only after checking the first words on this phone.',
  preparingToSpeak: 'Preparing to speak',
  readyToSpeak: 'Ready to speak',
  ready_sound_failed:
    'The ready-to-speak cue could not be played. Check your PC audio output.',
  feedback_sound_failed:
    'The feedback sound could not be played. Check your PC audio output.',
  failureSoundHelp:
    'Failures always use a distinct sound. Results remain available as ordinary text.',
  verification: 'Verification deadline (seconds)',
  saveSettings: 'Save settings',
  saved: 'Settings saved',
  reconnect: 'Reconnect',
  settingsHelp:
    'Authorize and connect one phone with ADB outside TakeDock. Changes to ADB and the deadline apply on reconnect.',
  checking: 'Checking…',
  loading: 'Loading…',
  available: 'Available',
  error: 'Error',
  noDevice: 'Connect one authorized phone to manage videos.',
  destination_required: 'Choose a computer destination folder in Settings.',
  destination_exists:
    'A file with that name already exists at the destination.',
  source_changed: 'The phone file changed. Refresh and try again.',
  video_not_finalized:
    'This video has not finished saving or its container is unrecognized.',
  copy_hash_mismatch:
    'The copied file did not match the original. The phone original was retained.',
  operation_cancelled: 'Operation cancelled',
  session_replaced: 'The phone session changed. Refresh before trying again.',
  media_index_pending:
    'Files changed successfully; Android media indexing may need a refresh.',
  command_shell_closed: 'The recording command connection stopped. Reconnect.',
  adb_not_found: 'ADB was not found. Set its executable path in Settings.',
} as const;
const ar: Record<keyof typeof en, string> = {
  duration: 'المدة',
  durationUnavailable: 'المدة غير متاحة',
  hours: 'ساعة',
  minutes: 'دقيقة',
  seconds: 'ثانية',
  milliseconds: 'مللي ثانية',
  destination_setup_failed:
    'تعذر إنشاء مجلد الحفظ الافتراضي. اختر مجلدًا من الإعدادات.',
  settings_save_failed: 'تعذر حفظ الإعدادات.',
  activity: 'نشاط البرنامج',
  activityProgress: 'تقدم العملية',
  copying: 'جارٍ نسخ الفيديو',
  verifying: 'جارٍ التحقق من الفيديو المنسوخ',
  deleting: 'جارٍ حذف الفيديو',
  renaming: 'جارٍ تغيير اسم الفيديو',
  preparing: 'جارٍ تجهيز العملية',
  loadingVideos: 'جارٍ تحميل قائمة الفيديوهات',
  windowName: 'اسم النافذة',
  renameWindow: 'حفظ اسم النافذة',
  newWindow: 'نافذة جديدة',
  window_renamed: 'تم حفظ اسم النافذة',
  windowHelp:
    'يفتح Ctrl+Shift+N نافذة أخرى. الهاتف ومجلد الحفظ والإعدادات مشتركة.',
  invalid_window_name: 'أدخل اسمًا للنافذة من حرف إلى 80 حرفًا دون محارف تحكم.',
  updates: 'التحديثات',
  updateHelp:
    'يُفحص التحديث تلقائيًا عند التشغيل. يبدأ التثبيت فقط عند اختيار تثبيت التحديث. التثبيت يغلق TakeDock ويلغي العمليات المعلقة.',
  checkUpdates: 'فحص التحديثات',
  installUpdate: 'تثبيت التحديث',
  update_checking: 'جارٍ فحص التحديثات…',
  update_available: 'يتوفر تحديث جديد.',
  update_current: 'لديك أحدث إصدار منشور.',
  update_installing: 'جارٍ تنزيل التحديث والتحقق منه…',
  update_installed: 'ثُبّت التحديث.',
  update_error: 'تعذرت عملية التحديث. يمكنك إعادة الفحص.',
  recording: 'التصوير',
  videos: 'الفيديوهات',
  settings: 'الإعدادات',
  start: 'بدء فيديو',
  stop: 'إيقاف',
  deleteTake: 'حذف هذه المحاولة',
  stopAndDelete: 'إيقاف وحذف',
  discardShortcutHelp:
    'الحذف يتطلب تفعيل زر إيقاف وحذف؛ الاختصار Ctrl+Shift+R يبدأ التسجيل أو يوقفه فقط.',
  discardAssumptionHelp:
    'الحذف التلقائي يتطلب أن يحفظ Open Camera في هذا المجلد وألّا يضيف تطبيق آخر فيديوهات أثناء المحاولة.',
  discard_waiting_to_stop: 'جارٍ انتظار جاهزية المحاولة للإيقاف…',
  discard_confirming_stop: 'جارٍ التأكد من توقف التسجيل…',
  discard_finalizing: 'جارٍ انتظار اكتمال حفظ الفيديو…',
  discard_deleting: 'جارٍ حذف هذه المحاولة…',
  discard_completed: 'حُذفت المحاولة.',
  discard_retained: 'لم يكتمل الحذف التلقائي.',
  discard_outcome_unknown:
    'صدر إذن الحذف، لكن تعذر تأكيد النتيجة. راجع الفيديوهات قبل اتخاذ إجراء آخر.',
  discard_no_managed_take:
    'الحذف التلقائي متاح فقط لمحاولة بدأ تسجيلها من TakeDock.',
  discard_ready_for_start:
    'ابدأ المحاولة من TakeDock لإتاحة حذفها من شاشة التسجيل.',
  discard_not_confirmed_idle:
    'لم تبدأ المحاولة من حالة كاميرا متوقفة مؤكدة. أوقف التسجيل بالطريقة العادية ثم حدد الفيديو من قائمة الفيديوهات.',
  discard_baseline_unavailable:
    'لم تجهز قائمة الملفات قبل بدء هذه المحاولة. أوقف التسجيل بالطريقة العادية ثم حدد الفيديو من قائمة الفيديوهات.',
  discard_file_history_lost:
    'تعذر ربط المحاولة بفيديو واحد لم يُستبدل. راجع الفيديوهات وحدد الملف بنفسك.',
  discard_take_replaced:
    'هذا الطلب يخص محاولة سابقة. راجع حالة التسجيل الحالية.',
  discard_capture_changed: 'ألغى إجراء تسجيل آخر الحذف المعلّق.',
  discard_observation_lost:
    'تعذر التأكد من حالة الكاميرا. لن يستمر الحذف المعلّق.',
  discard_start_unconfirmed:
    'لم يتأكد بدء التسجيل. لن تُحذف المحاولة تلقائيًا.',
  discard_stop_unconfirmed:
    'لم يتأكد توقف التسجيل. راجع الهاتف؛ أُلغي الحذف التلقائي.',
  discard_source_changed:
    'تغيّر الفيديو أثناء تجهيز الحذف. راجع قائمة الفيديوهات.',
  discard_inspection_failed:
    'تعذر فحص الفيديو للتأكد من إمكانية حذفه. راجع قائمة الفيديوهات.',
  discard_finalization_timeout:
    'لم يكتمل حفظ الفيديو في الوقت المحدد. راجع قائمة الفيديوهات قبل حذفه.',
  discard_cancelled: 'أُلغي الحذف المعلّق.',
  discard_disconnected: 'تغيّر الاتصال. لن يستمر الحذف المعلّق.',
  discard_worker_unavailable:
    'تعذر إدراج الحذف في الطابور. راجع قائمة الفيديوهات.',
  discard_capture_pending:
    'ما زال إجراء تسجيل في انتظار التأكيد. راجع حالة التسجيل قبل المحاولة مجددًا.',
  discard_busy: 'ما زالت عملية حذف سابقة قيد الإنهاء.',
  discard_delete_failed: 'تعذر حذف المحاولة. راجع قائمة الفيديوهات.',
  discard_permit_timeout:
    'لم يصدر إذن الحذف في الوقت المحدد. لم يُحذف الفيديو.',
  discard_permit_failed: 'تعذر الحصول على إذن الحذف. لم يُحذف الفيديو.',
  discard_invalid_operation: 'طلب الحذف غير صالح. لم يُحذف الفيديو.',
  pause: 'إيقاف مؤقت',
  resume: 'استئناف الفيديو',
  idle: 'جاهز',
  unknown: 'غير متاح',
  paused: 'متوقف مؤقتًا',
  recordingState: 'جارٍ التصوير',
  connection: 'اتصال الهاتف',
  connected: 'متصل',
  disconnected: 'لا يوجد هاتف متصل ومصرح به',
  ready: 'جاهز',
  open_camera_foreground:
    'افتح Open Camera على الهاتف واتركه في النافذة النشطة.',
  select_video_mode: 'اختر وضع الفيديو في Open Camera.',
  observer_starting: 'جارٍ بدء مراقب الكاميرا…',
  observer_setup_failed: 'مراقب الكاميرا غير متاح. يمكنك إدارة ملفات الفيديو.',
  multiple_devices: 'وصّل هاتفًا واحدًا مصرحًا به فقط.',
  verification_unconfirmed:
    'تعذر التحقق من أمر التصوير الأخير. افحص الهاتف ثم أعد الاتصال.',
  unsupported_camera_state:
    'حالة الكاميرا غير متاحة. افحص Open Camera وإعدادات أزرار الصوت.',
  observer_closed: 'توقف مراقب الكاميرا. أعد الاتصال لاستعادة التحكم.',
  command_delivery_failed: 'تعذر إرسال أمر التصوير.',
  captureHelp:
    'يجب أن يكون Open Camera نشطًا وفي وضع الفيديو، مع ضبط رفع الصوت للبدء والإنهاء وخفض الصوت للإيقاف المؤقت والاستئناف.',
  recordingKeys:
    'Ctrl+Shift+R: بدء أو إنهاء · Ctrl+Shift+P: إيقاف مؤقت أو استئناف',
  refresh: 'تحديث الفيديوهات',
  selectAll: 'تحديد كل الفيديوهات المكتملة',
  clear: 'إلغاء التحديد',
  copy: 'نسخ إلى الكمبيوتر',
  move: 'نقل إلى الكمبيوتر',
  delete: 'حذف من الهاتف',
  rename: 'إعادة تسمية الفيديو',
  name: 'الاسم',
  size: 'الحجم',
  modified: 'آخر تعديل',
  state: 'الحالة',
  select: 'تحديد',
  complete: 'مكتمل',
  protected: 'محمي: فيديو غير مكتمل أو تنسيق غير معروف',
  empty: 'لا توجد فيديوهات. حدّث القائمة أثناء اتصال الهاتف.',
  selected: 'المحدد',
  newName: 'الاسم الجديد بدون الامتداد',
  saveRename: 'حفظ الاسم',
  cancel: 'إلغاء',
  confirmDelete: 'تأكيد الحذف',
  deleteHelp: 'هل تريد حذف الفيديوهات المحددة نهائيًا من الهاتف؟',
  count: 'فيديوهات',
  jobs: 'عمليات الملفات',
  completed: 'اكتملت',
  failed: 'فشلت',
  cancelled: 'أُلغيت',
  skipped: 'لم تُنفذ',
  queued: 'في الانتظار',
  running: 'جارٍ التنفيذ',
  result: 'النتيجة',
  language: 'اللغة',
  destination: 'مجلد الحفظ على الكمبيوتر',
  chooseFolder: 'اختيار المجلد',
  adbPath: 'مسار برنامج ADB (اختياري)',
  chooseAdb: 'اختيار برنامج ADB',
  successSound: 'تشغيل صوت عند النجاح',
  readySound: 'تشغيل صفارة الجاهزية للكلام',
  readyDelay: 'مهلة الجاهزية للكلام بعد التأكيد (مللي ثانية)',
  readyDelayHelp:
    'تبدأ المهلة بعد تأكيد بدء التصوير، وتسري تغييراتها من اللقطة التالية. لا تقللها إلا بعد مراجعة أول كلماتك في تسجيل من هذا الهاتف.',
  preparingToSpeak: 'جارٍ التجهيز للكلام',
  readyToSpeak: 'جاهز للكلام',
  ready_sound_failed:
    'تعذر تشغيل صفارة الجاهزية للكلام. تحقق من مخرج الصوت في الكمبيوتر.',
  feedback_sound_failed:
    'تعذر تشغيل صوت التنبيه. تحقق من مخرج الصوت في الكمبيوتر.',
  failureSoundHelp: 'الفشل له صوت مختلف دائمًا. النتائج متاحة كنص عادي.',
  verification: 'مهلة التحقق بالثواني',
  saveSettings: 'حفظ الإعدادات',
  saved: 'حُفظت الإعدادات',
  reconnect: 'إعادة الاتصال',
  settingsHelp:
    'صرّح للهاتف ووصّله باستخدام ADB خارج TakeDock. تغييرات ADB والمهلة تسري عند إعادة الاتصال.',
  checking: 'جارٍ الفحص…',
  loading: 'جارٍ التحميل…',
  available: 'متاح',
  error: 'خطأ',
  noDevice: 'وصّل هاتفًا واحدًا مصرحًا به لإدارة الفيديوهات.',
  destination_required: 'اختر مجلد الحفظ على الكمبيوتر من الإعدادات.',
  destination_exists: 'يوجد ملف بنفس الاسم في الوجهة.',
  source_changed: 'تغير الملف على الهاتف. حدّث القائمة ثم حاول مجددًا.',
  video_not_finalized: 'لم يكتمل حفظ هذا الفيديو أو لم يُتعرف على تنسيقه.',
  copy_hash_mismatch: 'النسخة لا تطابق الأصل. بقي الأصل على الهاتف.',
  operation_cancelled: 'أُلغيت العملية',
  session_replaced: 'تغير اتصال الهاتف. حدّث القائمة قبل المحاولة.',
  media_index_pending:
    'تغيرت الملفات بنجاح؛ قد يحتاج فهرس وسائط Android إلى تحديث.',
  command_shell_closed: 'انقطع مسار أوامر التصوير. أعد الاتصال.',
  adb_not_found: 'لم يُعثر على ADB. حدد مساره من الإعدادات.',
};
export function t(language: 'en' | 'ar', key: string): string {
  return (language === 'ar' ? ar : en)[key as keyof typeof en] ?? key;
}
